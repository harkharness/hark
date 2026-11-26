//! What the "add a project" field suggests while a path is being typed.
//! Nobody remembers a project's full directory: the field reads what was
//! typed two ways. With a separator in it, it is a path, and the folders
//! of the directory typed so far are offered. Without one, it is a name,
//! looked for in the places projects already live. No I/O here: the
//! adapter lists directories, this decides what to ask for and what comes
//! first.

/// How the typed text is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Query {
    /// `dir` is listed (it ends with a separator, `~` still unexpanded)
    /// and its folders are matched against `prefix`.
    Path { dir: String, prefix: String },
    /// A bare name, looked for under the known roots.
    Name(String),
}

/// One folder offered, before the adapter dresses it for the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub name: String,
    pub path: String,
    /// Levels below the root it was found under: 1 is a direct child.
    pub depth: usize,
    pub git: bool,
}

fn is_sep(c: char) -> bool {
    c == '/' || c == '\\'
}

/// A path as soon as it has a separator or starts at home; relative ones
/// start at home, the only place a relative path could sensibly mean.
pub fn parse(typed: &str) -> Query {
    let typed = typed.trim();
    if typed == "~" {
        return Query::Path { dir: "~/".into(), prefix: String::new() };
    }
    let Some(cut) = typed.rfind(is_sep) else {
        return Query::Name(typed.to_string());
    };
    let (dir, prefix) = typed.split_at(cut + 1);
    let rooted = dir.starts_with('~') || dir.starts_with(is_sep) || dir.chars().nth(1) == Some(':');
    let dir = if rooted { dir.to_string() } else { format!("~/{dir}") };
    Query::Path { dir, prefix: prefix.to_string() }
}

/// How well a folder name answers what was typed: the same name, then a
/// name that starts with it, then one that merely holds it.
fn name_score(name: &str, typed: &str) -> Option<u8> {
    let name = name.to_lowercase();
    let typed = typed.to_lowercase();
    if typed.is_empty() || name == typed {
        Some(3)
    } else if name.starts_with(&typed) {
        Some(2)
    } else if name.contains(&typed) {
        Some(1)
    } else {
        None
    }
}

/// Hidden folders stay out until the person types the dot themselves.
fn hidden_ok(name: &str, typed: &str) -> bool {
    !name.starts_with('.') || typed.starts_with('.')
}

/// The folders of one directory that answer `prefix`, best first.
pub fn rank_names(names: &[String], prefix: &str, limit: usize) -> Vec<String> {
    let mut hits: Vec<(u8, &String)> = names
        .iter()
        .filter(|n| hidden_ok(n, prefix))
        .filter_map(|n| name_score(n, prefix).map(|s| (s, n)))
        .collect();
    hits.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
    });
    hits.into_iter().take(limit).map(|(_, n)| n.clone()).collect()
}

/// Folders found under the roots that answer a bare name. The match comes
/// first, then a repository over a plain folder, then the shallower one:
/// a project is usually a repository near the top.
pub fn rank_found(found: Vec<Candidate>, typed: &str, limit: usize) -> Vec<Candidate> {
    let mut hits: Vec<(u8, Candidate)> = found
        .into_iter()
        .filter(|c| hidden_ok(&c.name, typed))
        .filter_map(|c| name_score(&c.name, typed).map(|s| (s, c)))
        .collect();
    hits.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.git.cmp(&a.1.git))
            .then_with(|| a.1.depth.cmp(&b.1.depth))
            .then_with(|| a.1.path.to_lowercase().cmp(&b.1.path.to_lowercase()))
    });
    let mut seen = std::collections::HashSet::new();
    hits.into_iter()
        .map(|(_, c)| c)
        .filter(|c| seen.insert(c.path.clone()))
        .take(limit)
        .collect()
}

/// A path as the field shows it: home becomes `~`.
pub fn tilde(path: &str, home: &str) -> String {
    let home = home.trim_end_matches(is_sep);
    if home.is_empty() {
        return path.to_string();
    }
    match path.strip_prefix(home) {
        Some("") => "~".to_string(),
        Some(rest) if rest.starts_with(is_sep) => format!("~{rest}"),
        _ => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn cand(path: &str, depth: usize, git: bool) -> Candidate {
        Candidate {
            name: path.rsplit('/').next().unwrap().to_string(),
            path: path.to_string(),
            depth,
            git,
        }
    }

    #[test]
    fn text_with_a_separator_is_a_directory_and_what_comes_after_it() {
        assert_eq!(
            parse("~/Projects/ha"),
            Query::Path { dir: "~/Projects/".into(), prefix: "ha".into() }
        );
        assert_eq!(
            parse("/srv/apps/"),
            Query::Path { dir: "/srv/apps/".into(), prefix: String::new() }
        );
    }

    #[test]
    fn a_lone_tilde_lists_home() {
        assert_eq!(parse("~"), Query::Path { dir: "~/".into(), prefix: String::new() });
    }

    #[test]
    fn a_relative_path_starts_at_home() {
        assert_eq!(
            parse("Projects/ha"),
            Query::Path { dir: "~/Projects/".into(), prefix: "ha".into() }
        );
    }

    #[test]
    fn text_without_a_separator_is_a_name_to_look_for() {
        assert_eq!(parse("  landing "), Query::Name("landing".into()));
        assert_eq!(parse(""), Query::Name(String::new()));
    }

    #[test]
    fn a_name_that_starts_with_the_text_beats_one_that_only_holds_it() {
        let got = rank_names(&names(&["old-hark", "Hark", "harbor", "notes"]), "har", 10);
        assert_eq!(got, names(&["harbor", "Hark", "old-hark"]));
    }

    #[test]
    fn the_same_name_comes_first_whatever_the_case() {
        let got = rank_names(&names(&["hark-site", "HARK", "hark2"]), "hark", 10);
        assert_eq!(got[0], "HARK");
    }

    #[test]
    fn nothing_typed_lists_the_directory_in_order_and_capped() {
        let got = rank_names(&names(&["zeta", "Alpha", "beta"]), "", 2);
        assert_eq!(got, names(&["Alpha", "beta"]));
    }

    #[test]
    fn hidden_folders_wait_for_the_dot() {
        let all = names(&[".config", "code", ".cache"]);
        assert_eq!(rank_names(&all, "c", 10), names(&["code"]));
        assert_eq!(rank_names(&all, ".c", 10), names(&[".cache", ".config"]));
    }

    #[test]
    fn a_found_repository_beats_a_plain_folder_and_shallow_beats_deep() {
        let got = rank_found(
            vec![
                cand("/w/archive/site", 2, true),
                cand("/w/site", 1, false),
                cand("/w/site-next", 1, true),
                cand("/w/apps/site", 2, true),
                cand("/w/other", 1, true),
            ],
            "site",
            10,
        );
        let paths: Vec<&str> = got.iter().map(|c| c.path.as_str()).collect();
        // Exact names first (repositories, then the plain one), the
        // prefix match after them; "other" does not answer at all.
        assert_eq!(paths, vec!["/w/apps/site", "/w/archive/site", "/w/site", "/w/site-next"]);
    }

    #[test]
    fn a_folder_reached_through_two_roots_is_offered_once() {
        let got = rank_found(vec![cand("/w/site", 1, true), cand("/w/site", 2, true)], "site", 10);
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn home_is_shown_as_a_tilde_and_only_at_a_path_boundary() {
        assert_eq!(tilde("/home/u/Projects/x", "/home/u"), "~/Projects/x");
        assert_eq!(tilde("/home/u", "/home/u/"), "~");
        assert_eq!(tilde("/home/user2/x", "/home/u"), "/home/user2/x");
        assert_eq!(tilde("/srv/x", "/home/u"), "/srv/x");
    }
}

//! Where a space keeps its pages.
//!
//! Pages began in one place, `~/.hark/artifacts/<space>/`: private to the
//! machine, seen by nobody else, lost with the laptop. A project's pages
//! are the project's documentation, and documentation belongs with the
//! code: versioned, reviewed, read by a colleague who cloned the
//! repository. So a project can keep its pages inside itself, and git
//! becomes the way they travel, before any hosting exists.
//!
//! The decisions, and why:
//!
//! - **`docs/hark/`, not `.hark/`.** A visible folder reads as
//!   documentation, and its markdown renders on the forge for someone
//!   without Hark. `.hark/` in a project is the machine's own state (the
//!   journal, the dispatch log, the briefs) and stays ignored.
//! - **A folder of Hark's own, not the root of `docs/`.** `docs/` usually
//!   has an owner already: a docs site with its build and its gates, or a
//!   folder a forge publishes as pages. A page an agent wrote, landing
//!   there, breaks the build or goes online without anyone deciding. And
//!   in a folder of its own everything is a page: nothing is guessed, and
//!   a document someone wrote by hand is never edited by mistake.
//! - **The folder is the choice.** A project whose `docs/hark/` exists
//!   keeps its pages there; one without it keeps them in the machine's
//!   root. Nothing is written into a repository until its owner asks
//!   ("move to the repository"), and a colleague who clones a repository
//!   that has the folder sees the pages with no setting to copy.
//! - **Only what is documentation travels.** The pages and the space's
//!   index (their titles, order and ties) go to the repository. What is
//!   the machine's stays in the root: the versions Hark keeps while
//!   editing (git is the repository's history), a dashboard's cache, the
//!   comments (yours until you send one to a chat).
//! - **Pages with no project stay where they were**: the personal space,
//!   and whatever Hark's own chat writes.
//!
//! What a repository brings is not trusted for coming from one: a page is
//! read through the same rules as before (no path that climbs, nothing
//! through a symlink, html in a frame that cannot reach the app), and a
//! dashboard's command still runs only when the person asks.
//!
//! Known cost: a sheet, a board and a dashboard are JSON, and two people
//! editing the same one meet in a merge conflict. Markdown and html merge
//! like any text.

use super::artifacts::validate_path;

/// The folder, inside a project, where its pages live.
pub const IN_PROJECT: &str = "docs/hark";

/// A space whose pages live inside its project.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Home {
    pub space: String,
    /// The pages' folder, absolute: `<project>/docs/hark`.
    pub dir: String,
}

/// The space and the page a file is, wherever pages may live: inside a
/// project's own folder, or in a space of the machine's root
/// (`<root>/<space>/<page>`). None for anything that is not a page.
pub fn page_at(file: &str, root: &str, homes: &[Home]) -> Option<(String, String)> {
    // The longest folder first: a project inside another keeps its own.
    let mut by_depth: Vec<&Home> = homes.iter().collect();
    by_depth.sort_by_key(|h| std::cmp::Reverse(h.dir.len()));
    for home in by_depth {
        if let Some(path) = inside(file, &home.dir) {
            return validate_path(path).ok().map(|()| (home.space.clone(), path.to_string()));
        }
    }
    let (space, path) = inside(file, root)?.split_once('/')?;
    if space.is_empty() || space == "." || space == ".." {
        return None;
    }
    validate_path(path).ok()?;
    Some((space.to_string(), path.to_string()))
}

/// The pages folder of the project a session works in, when that project
/// keeps its pages in itself: where the agent is told to write. The
/// project is the folder above `docs/hark`; a session anywhere under it
/// is in it, and the innermost project wins.
pub fn home_of<'a>(cwd: &str, homes: &'a [Home]) -> Option<&'a Home> {
    let cwd = cwd.trim_end_matches('/');
    homes
        .iter()
        .filter_map(|home| {
            let project = home.dir.trim_end_matches('/').strip_suffix(IN_PROJECT)?.trim_end_matches('/');
            (cwd == project || inside(cwd, project).is_some()).then_some((project.len(), home))
        })
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, home)| home)
}

/// What is left of `file` under `dir`, when it is under it.
fn inside<'a>(file: &'a str, dir: &str) -> Option<&'a str> {
    file.strip_prefix(dir.trim_end_matches('/'))?.strip_prefix('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = "/Users/x/.hark/artifacts";

    fn homes() -> Vec<Home> {
        vec![Home { space: "tulipafin".into(), dir: "/Users/x/code/tulipafin/docs/hark".into() }]
    }

    fn at(file: &str) -> Option<(String, String)> {
        page_at(file, ROOT, &homes())
    }

    #[test]
    fn a_file_in_a_projects_own_folder_is_a_page_of_that_projects_space() {
        assert_eq!(
            at("/Users/x/code/tulipafin/docs/hark/decisoes/adr-004.md"),
            Some(("tulipafin".into(), "decisoes/adr-004.md".into()))
        );
    }

    #[test]
    fn a_file_in_a_space_of_the_machines_root_is_a_page_as_before() {
        assert_eq!(at("/Users/x/.hark/artifacts/pessoal/notas.md"), Some(("pessoal".into(), "notas.md".into())));
    }

    #[test]
    fn the_rest_of_the_projects_docs_is_not_harks() {
        // A document someone wrote by hand, beside the folder.
        assert_eq!(at("/Users/x/code/tulipafin/docs/guia.md"), None);
        assert_eq!(at("/Users/x/code/tulipafin/README.md"), None);
        assert_eq!(at("/Users/x/code/tulipafin/docs/hark-old/a.md"), None, "a sibling with the same prefix is outside");
    }

    #[test]
    fn a_project_with_no_folder_of_its_own_has_no_pages_in_it() {
        assert_eq!(at("/Users/x/code/horta/docs/hark/a.md"), None);
    }

    #[test]
    fn what_is_not_a_page_is_not_one_in_either_place() {
        assert_eq!(at("/Users/x/code/tulipafin/docs/hark/img/a.png"), None);
        assert_eq!(at("/Users/x/code/tulipafin/docs/hark/index.json"), None, "the index is the tree's");
        assert_eq!(at("/Users/x/code/tulipafin/docs/hark/../../.env.md"), None, "no climbing out of the folder");
        assert_eq!(at("/Users/x/code/tulipafin/docs/hark"), None);
        assert_eq!(at("/Users/x/.hark/artifacts/solto.md"), None, "in the root a page lives in a space");
        assert_eq!(at("/Users/x/.hark/artifacts/../x/a.md"), None);
    }

    #[test]
    fn the_folder_and_the_root_are_the_same_with_a_slash_at_the_end() {
        let homes = vec![Home { space: "t".into(), dir: "/p/t/docs/hark/".into() }];
        assert_eq!(page_at("/p/t/docs/hark/a.md", "/r/", &homes), Some(("t".into(), "a.md".into())));
        assert_eq!(page_at("/r/pessoal/a.md", "/r/", &homes), Some(("pessoal".into(), "a.md".into())));
    }

    #[test]
    fn a_session_in_a_project_writes_to_that_projects_folder() {
        let homes = homes();
        assert_eq!(home_of("/Users/x/code/tulipafin", &homes).map(|h| h.space.as_str()), Some("tulipafin"));
        assert_eq!(home_of("/Users/x/code/tulipafin/apps/api", &homes).map(|h| h.space.as_str()), Some("tulipafin"), "a folder under the project is the project");
        assert_eq!(home_of("/Users/x/code/tulipafin/", &homes).map(|h| h.space.as_str()), Some("tulipafin"));
    }

    #[test]
    fn a_session_elsewhere_has_no_project_folder_to_write_to() {
        let homes = homes();
        assert_eq!(home_of("/Users/x/code/horta", &homes), None);
        assert_eq!(home_of("/Users/x/code/tulipafin-site", &homes), None, "a sibling with the same prefix is another project");
        assert_eq!(home_of("/Users/x/code", &homes), None, "the folder above the project is not it");
    }

    #[test]
    fn a_session_in_a_project_inside_another_writes_to_the_inner_one() {
        let homes = vec![
            Home { space: "mono".into(), dir: "/p/mono/docs/hark".into() },
            Home { space: "api".into(), dir: "/p/mono/apps/api/docs/hark".into() },
        ];
        assert_eq!(home_of("/p/mono/apps/api/src", &homes).map(|h| h.space.as_str()), Some("api"));
        assert_eq!(home_of("/p/mono/apps/web", &homes).map(|h| h.space.as_str()), Some("mono"));
    }

    #[test]
    fn a_project_inside_another_keeps_its_own_pages() {
        let homes = vec![
            Home { space: "mono".into(), dir: "/p/mono/docs/hark".into() },
            Home { space: "api".into(), dir: "/p/mono/apps/api/docs/hark".into() },
        ];
        assert_eq!(page_at("/p/mono/apps/api/docs/hark/a.md", "/r", &homes), Some(("api".into(), "a.md".into())));
        assert_eq!(page_at("/p/mono/docs/hark/a.md", "/r", &homes), Some(("mono".into(), "a.md".into())));
    }
}

//! Artifacts on disk: `<root>/<space>/<page>`, or, for a project that
//! keeps its pages in itself, `<project>/docs/hark/<page>`
//! (`domain::pages_home` says why and what goes where). Thin: every path
//! is checked by the domain (`validate_path`) and the one rule only the
//! disk can enforce lives here: never through a symlink, never outside
//! the folder a space was given.

use crate::domain::page_history;
use crate::domain::pages_home::{Home, IN_PROJECT};
use crate::ports::{ArtifactStore, PageVersion};
use std::path::{Path, PathBuf};

/// Where a space keeps its pages' versions: hidden, so never a page.
const HISTORY_DIR: &str = ".history";

pub struct FsArtifacts {
    root: PathBuf,
    /// The spaces whose pages live in their project: space, folder.
    homes: Vec<(String, PathBuf)>,
}

fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}

fn hidden(name: &std::ffi::OsStr) -> bool {
    name.to_string_lossy().starts_with('.')
}

/// A space is one folder name: no slashes, not hidden, not a climb.
fn validate_space(space: &str) -> anyhow::Result<()> {
    if space.is_empty() || space.contains('/') || space.contains('\\') || space.starts_with('.') {
        anyhow::bail!("{space}: not a space name");
    }
    Ok(())
}

/// The project's pages folder, as a path: `<project>/docs/hark`, with
/// every step of the way checked against being a link. A repository can
/// ship `docs -> /` or `docs/hark -> ~/.ssh`; a link is an error here,
/// never a place.
fn home_path(project: &Path) -> anyhow::Result<PathBuf> {
    let mut cur = project.to_path_buf();
    for seg in IN_PROJECT.split('/') {
        cur.push(seg);
        if is_symlink(&cur) {
            anyhow::bail!("refused: {} is a symlink", cur.display());
        }
    }
    Ok(cur)
}

/// A project's own pages folder, when it has one: a real folder, reached
/// through real folders. Its existing is the project's choice to keep
/// its pages in itself.
pub fn project_home(project: &Path) -> Option<PathBuf> {
    home_path(project).ok().filter(|dir| dir.is_dir())
}

/// Move a space's pages and its index from the machine's root into its
/// project, making the folder that is the project's choice. What is the
/// machine's (versions, caches, comments) stays in the root. Returns how
/// many pages moved.
///
/// Nothing is written over: a page already in the project with other
/// text, or an index of the project's own, stops the move before it
/// starts. Every page is copied before any is removed, so a move that
/// fails halfway leaves the root whole.
pub fn move_to_project(root: &Path, space: &str, project: &Path) -> anyhow::Result<usize> {
    if !project.is_dir() {
        anyhow::bail!("{}: not a folder", project.display());
    }
    let from = FsArtifacts::new(root);
    let pages = from.list_pages(space)?;
    let index = from.read_index(space)?;
    let home = home_path(project)?;

    let mut texts = Vec::with_capacity(pages.len());
    for page in &pages {
        let text = from.read_page(space, page)?;
        let there = home.join(page);
        if is_symlink(&there) || (there.exists() && std::fs::read_to_string(&there).ok().as_deref() != Some(text.as_str())) {
            anyhow::bail!("{page}: the project already has another page there");
        }
        texts.push(text);
    }
    let index_there = home.join("index.json");
    if !index.is_empty() && index_there.exists() && std::fs::read_to_string(&index_there).ok().as_deref() != Some(index.as_str()) {
        anyhow::bail!("index.json: the project already has pages of its own, organized another way");
    }

    std::fs::create_dir_all(&home)?;
    let to = FsArtifacts { root: root.to_path_buf(), homes: vec![(space.to_string(), home)] };
    for (page, text) in pages.iter().zip(&texts) {
        to.write_page(space, page, text)?;
    }
    if !index.is_empty() {
        to.write_index(space, &index)?;
    }
    for page in &pages {
        from.remove_page(space, page)?;
    }
    if !index.is_empty() {
        std::fs::remove_file(from.space_dir(space)?.join("index.json"))?;
    }
    Ok(pages.len())
}

impl FsArtifacts {
    /// Every space in the machine's root.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), homes: vec![] }
    }

    /// The machine's root, and the projects (space name, folder): the
    /// ones that have a pages folder of their own keep their pages there.
    pub fn with_projects(root: impl Into<PathBuf>, projects: &[(String, PathBuf)]) -> Self {
        let homes = projects
            .iter()
            .filter(|(space, _)| validate_space(space).is_ok())
            .filter_map(|(space, project)| Some((space.clone(), project_home(project)?)))
            .collect();
        Self { root: root.into(), homes }
    }

    /// Where a space's pages are on disk, to show and to tell an agent.
    pub fn folder(&self, space: &str) -> anyhow::Result<PathBuf> {
        self.pages_dir(space)
    }

    /// Which spaces live in their project, and where.
    pub fn homes(&self) -> Vec<Home> {
        self.homes.iter().map(|(space, dir)| Home { space: space.clone(), dir: dir.to_string_lossy().to_string() }).collect()
    }

    /// The root folder, made real when missing; never a link.
    fn root_dir(&self) -> anyhow::Result<&Path> {
        if is_symlink(&self.root) {
            anyhow::bail!("refused: {} is a symlink", self.root.display());
        }
        if !self.root.is_dir() {
            std::fs::create_dir_all(&self.root)?;
        }
        Ok(&self.root)
    }

    fn space_dir(&self, space: &str) -> anyhow::Result<PathBuf> {
        validate_space(space)?;
        let dir = self.root_dir()?.join(space);
        if is_symlink(&dir) {
            anyhow::bail!("refused: {} is a symlink", dir.display());
        }
        Ok(dir)
    }

    /// Where a space's pages and index live: its project's own folder
    /// when it has one, else the space's folder in the root (which also
    /// keeps, for every space, what is the machine's).
    fn pages_dir(&self, space: &str) -> anyhow::Result<PathBuf> {
        validate_space(space)?;
        match self.homes.iter().find(|(name, _)| name == space) {
            Some((_, dir)) if is_symlink(dir) => anyhow::bail!("refused: {} is a symlink", dir.display()),
            Some((_, dir)) => Ok(dir.clone()),
            None => self.space_dir(space),
        }
    }

    /// The page's absolute path, with every EXISTING component checked
    /// against being a link. Components not there yet are fine: they are
    /// about to be created as real folders.
    fn page_path(&self, space: &str, path: &str) -> anyhow::Result<PathBuf> {
        crate::domain::artifacts::validate_path(path).map_err(anyhow::Error::msg)?;
        let mut cur = self.pages_dir(space)?;
        for seg in path.split('/') {
            cur.push(seg);
            if is_symlink(&cur) {
                anyhow::bail!("refused: {} is a symlink", cur.display());
            }
        }
        Ok(cur)
    }

    /// The folder of a page's versions: `<space>/.history/<page path>/`.
    fn history_dir(&self, space: &str, path: &str) -> anyhow::Result<PathBuf> {
        crate::domain::artifacts::validate_path(path).map_err(anyhow::Error::msg)?;
        let mut cur = self.space_dir(space)?.join(HISTORY_DIR);
        for seg in path.split('/') {
            cur.push(seg);
            if is_symlink(&cur) {
                anyhow::bail!("refused: {} is a symlink", cur.display());
            }
        }
        Ok(cur)
    }

    fn version_file(&self, space: &str, path: &str, at: &str) -> anyhow::Result<PathBuf> {
        let name = page_history::file_name(at);
        if name.is_empty() || name.contains('/') || name.starts_with('.') {
            anyhow::bail!("{at}: not a version");
        }
        Ok(self.history_dir(space, path)?.join(name))
    }

    /// A non-page file inside the space (a cache): relative, no climb,
    /// never through a link, never the index.
    fn aside_path(&self, space: &str, path: &str) -> anyhow::Result<PathBuf> {
        if path.is_empty() || path.starts_with('/') || path.contains('\\') || path == "index.json" {
            anyhow::bail!("{path}: not a file of the space");
        }
        if path.split('/').any(|seg| seg.is_empty() || seg == "." || seg == "..") {
            anyhow::bail!("{path}: no empty, `.` or `..` segments");
        }
        let mut cur = self.space_dir(space)?;
        for seg in path.split('/') {
            cur.push(seg);
            if is_symlink(&cur) {
                anyhow::bail!("refused: {} is a symlink", cur.display());
            }
        }
        Ok(cur)
    }

    fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) -> anyhow::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            if hidden(&name) {
                continue;
            }
            let path = entry.path();
            if is_symlink(&path) {
                continue;
            }
            let here = if rel.is_empty() { name.to_string_lossy().to_string() } else { format!("{rel}/{}", name.to_string_lossy()) };
            if path.is_dir() {
                Self::walk(&path, &here, out)?;
            } else if !crate::domain::artifacts::ext_of(&here).is_empty() {
                out.push(here);
            }
        }
        Ok(())
    }
}

impl ArtifactStore for FsArtifacts {
    fn spaces(&self) -> anyhow::Result<Vec<String>> {
        let mut out = vec![];
        for entry in std::fs::read_dir(self.root_dir()?)? {
            let entry = entry?;
            let path = entry.path();
            if hidden(&entry.file_name()) || is_symlink(&path) || !path.is_dir() {
                continue;
            }
            out.push(entry.file_name().to_string_lossy().to_string());
        }
        // A space that lives in its project exists with no folder here.
        out.extend(self.homes.iter().map(|(space, _)| space.clone()));
        out.sort();
        out.dedup();
        Ok(out)
    }

    fn read_index(&self, space: &str) -> anyhow::Result<String> {
        let file = self.pages_dir(space)?.join("index.json");
        if is_symlink(&file) {
            anyhow::bail!("refused: {} is a symlink", file.display());
        }
        match std::fs::read_to_string(&file) {
            Ok(s) => Ok(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(e.into()),
        }
    }

    fn write_index(&self, space: &str, json: &str) -> anyhow::Result<()> {
        let dir = self.pages_dir(space)?;
        std::fs::create_dir_all(&dir)?;
        let file = dir.join("index.json");
        if is_symlink(&file) {
            anyhow::bail!("refused: {} is a symlink", file.display());
        }
        std::fs::write(file, json)?;
        Ok(())
    }

    fn list_pages(&self, space: &str) -> anyhow::Result<Vec<String>> {
        let dir = self.pages_dir(space)?;
        let mut out = vec![];
        if dir.is_dir() {
            Self::walk(&dir, "", &mut out)?;
        }
        out.sort();
        Ok(out)
    }

    fn read_page(&self, space: &str, path: &str) -> anyhow::Result<String> {
        Ok(std::fs::read_to_string(self.page_path(space, path)?)?)
    }

    fn write_page(&self, space: &str, path: &str, text: &str) -> anyhow::Result<()> {
        let file = self.page_path(space, path)?;
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(file, text)?;
        Ok(())
    }

    fn remove_page(&self, space: &str, path: &str) -> anyhow::Result<()> {
        let file = self.page_path(space, path)?;
        if !file.is_file() {
            anyhow::bail!("{path}: not found");
        }
        std::fs::remove_file(file)?;
        Ok(())
    }

    fn absolute(&self, space: &str, path: &str) -> anyhow::Result<PathBuf> {
        self.page_path(space, path)
    }

    fn read_aside(&self, space: &str, path: &str) -> anyhow::Result<Option<String>> {
        let file = self.aside_path(space, path)?;
        match std::fs::read_to_string(&file) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn write_aside(&self, space: &str, path: &str, text: &str) -> anyhow::Result<()> {
        let file = self.aside_path(space, path)?;
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(file, text)?;
        Ok(())
    }

    fn list_versions(&self, space: &str, path: &str) -> anyhow::Result<Vec<PageVersion>> {
        let dir = self.history_dir(space, path)?;
        let mut out = vec![];
        if !dir.is_dir() {
            return Ok(out);
        }
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let file = entry.path();
            if hidden(&entry.file_name()) || is_symlink(&file) || !file.is_file() {
                continue;
            }
            let bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
            out.push(PageVersion { at: page_history::instant(&entry.file_name().to_string_lossy()), bytes });
        }
        out.sort_by(|a, b| a.at.cmp(&b.at));
        Ok(out)
    }

    fn read_version(&self, space: &str, path: &str, at: &str) -> anyhow::Result<String> {
        let file = self.version_file(space, path, at)?;
        if is_symlink(&file) || !file.is_file() {
            anyhow::bail!("{at}: no such version of {path}");
        }
        Ok(std::fs::read_to_string(file)?)
    }

    fn write_version(&self, space: &str, path: &str, at: &str, text: &str) -> anyhow::Result<()> {
        let file = self.version_file(space, path, at)?;
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(file, text)?;
        Ok(())
    }

    fn remove_version(&self, space: &str, path: &str, at: &str) -> anyhow::Result<()> {
        let file = self.version_file(space, path, at)?;
        if !file.is_file() {
            anyhow::bail!("{at}: no such version of {path}");
        }
        std::fs::remove_file(file)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, FsArtifacts) {
        let dir = tempfile::tempdir().unwrap();
        let store = FsArtifacts::new(dir.path().join("artifacts"));
        (dir, store)
    }

    #[test]
    fn a_page_round_trips_and_its_folders_are_made_on_the_way() {
        let (_d, s) = store();
        s.write_page("hark", "decisoes/adr-004.md", "# ADR 004\n").unwrap();
        assert_eq!(s.read_page("hark", "decisoes/adr-004.md").unwrap(), "# ADR 004\n");
        assert_eq!(s.spaces().unwrap(), vec!["hark"]);
        assert_eq!(s.list_pages("hark").unwrap(), vec!["decisoes/adr-004.md"]);
        assert!(s.absolute("hark", "decisoes/adr-004.md").unwrap().ends_with("artifacts/hark/decisoes/adr-004.md"));
    }

    #[test]
    fn the_index_is_empty_until_written() {
        let (_d, s) = store();
        assert_eq!(s.read_index("hark").unwrap(), "");
        s.write_index("hark", "{}\n").unwrap();
        assert_eq!(s.read_index("hark").unwrap(), "{}\n");
        assert!(s.list_pages("hark").unwrap().is_empty(), "the index is not a page");
    }

    #[test]
    fn listing_skips_hidden_files_and_non_pages_and_sorts() {
        let (_d, s) = store();
        s.write_page("hark", "b.md", "").unwrap();
        s.write_page("hark", "a/x.html", "").unwrap();
        std::fs::write(s.absolute("hark", "a/x.html").unwrap().with_file_name("img.png"), b"").unwrap();
        std::fs::write(s.absolute("hark", "b.md").unwrap().with_file_name(".DS_Store"), b"").unwrap();
        assert_eq!(s.list_pages("hark").unwrap(), vec!["a/x.html", "b.md"]);
    }

    #[test]
    fn the_json_kinds_are_listed_and_the_index_never_is() {
        let (_d, s) = store();
        s.write_page("hark", "custos.sheet.json", "{}").unwrap();
        s.write_page("hark", "p/plano.board.json", "{}").unwrap();
        s.write_page("hark", "uso.dash.json", "{}").unwrap();
        s.write_index("hark", "{}\n").unwrap();
        std::fs::write(s.absolute("hark", "uso.dash.json").unwrap().with_file_name("dados.json"), b"{}").unwrap();
        assert_eq!(s.list_pages("hark").unwrap(), vec!["custos.sheet.json", "p/plano.board.json", "uso.dash.json"]);
        assert!(s.read_page("hark", "dados.json").is_err(), "a plain json is not a page");
    }

    #[test]
    fn a_path_that_climbs_or_a_bad_space_is_refused_before_the_disk_is_touched() {
        let (_d, s) = store();
        assert!(s.write_page("hark", "../x.md", "").is_err());
        assert!(s.write_page("../hark", "x.md", "").is_err());
        assert!(s.write_page("a/b", "x.md", "").is_err());
        assert!(s.write_page(".hidden", "x.md", "").is_err());
        assert!(s.read_page("hark", "notas.txt").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_inside_the_space_is_never_followed() {
        let (d, s) = store();
        s.write_page("hark", "ok.md", "").unwrap();
        let outside = d.path().join("outside.md");
        std::fs::write(&outside, "secret").unwrap();
        std::os::unix::fs::symlink(&outside, s.absolute("hark", "ok.md").unwrap().with_file_name("link.md")).unwrap();
        assert!(s.read_page("hark", "link.md").is_err());
        assert!(s.write_page("hark", "link.md", "x").is_err());
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "secret");
        assert_eq!(s.list_pages("hark").unwrap(), vec!["ok.md"], "links are not listed either");
        let dir_link = d.path().join("artifacts").join("hark").join("linked");
        std::os::unix::fs::symlink(d.path(), &dir_link).unwrap();
        assert!(s.read_page("hark", "linked/outside.md").is_err());
    }

    #[test]
    fn versions_live_hidden_beside_the_page_and_list_oldest_first() {
        let (_d, s) = store();
        s.write_page("hark", "notas/custos.sheet.json", "{}").unwrap();
        s.write_version("hark", "notas/custos.sheet.json", "2026-10-28T10:05:00Z", "b").unwrap();
        s.write_version("hark", "notas/custos.sheet.json", "2026-10-28T10:00:00Z", "a").unwrap();
        let list = s.list_versions("hark", "notas/custos.sheet.json").unwrap();
        assert_eq!(list.iter().map(|v| v.at.as_str()).collect::<Vec<_>>(), vec!["2026-10-28T10:00:00Z", "2026-10-28T10:05:00Z"]);
        assert_eq!(list[0].bytes, 1);
        assert_eq!(s.read_version("hark", "notas/custos.sheet.json", "2026-10-28T10:00:00Z").unwrap(), "a");
        assert_eq!(s.list_pages("hark").unwrap(), vec!["notas/custos.sheet.json"], "versions are not pages");
        s.remove_version("hark", "notas/custos.sheet.json", "2026-10-28T10:00:00Z").unwrap();
        assert_eq!(s.list_versions("hark", "notas/custos.sheet.json").unwrap().len(), 1);
        assert!(s.read_version("hark", "notas/custos.sheet.json", "nope").is_err());
        assert!(s.list_versions("hark", "outra.md").unwrap().is_empty(), "no history is an empty list");
    }

    #[test]
    fn a_file_beside_the_pages_is_read_and_written_but_never_listed_as_a_page() {
        let (_d, s) = store();
        assert_eq!(s.read_aside("hark", "uso.dash.cache.json").unwrap(), None);
        s.write_aside("hark", "uso.dash.cache.json", "{}").unwrap();
        assert_eq!(s.read_aside("hark", "uso.dash.cache.json").unwrap().as_deref(), Some("{}"));
        assert!(s.list_pages("hark").unwrap().is_empty());
        assert!(s.write_aside("hark", "../x.json", "").is_err());
        assert!(s.write_aside("hark", "index.json", "").is_err(), "the index is the tree's, not a cache");
    }

    // ---- a project that keeps its pages in itself (domain::pages_home) ----

    /// A machine root, a project with the folder, and the store over both.
    fn with_project() -> (tempfile::TempDir, PathBuf, PathBuf, FsArtifacts) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("artifacts");
        let project = dir.path().join("code").join("tulipafin");
        std::fs::create_dir_all(project.join("docs").join("hark")).unwrap();
        let store = FsArtifacts::with_projects(&root, &[("tulipafin".into(), project.clone())]);
        (dir, root, project, store)
    }

    fn files_under(dir: &Path) -> Vec<String> {
        fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
            let Ok(entries) = std::fs::read_dir(dir) else { return };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, base, out);
                } else {
                    out.push(path.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
                }
            }
        }
        let mut out = vec![];
        walk(dir, dir, &mut out);
        out.sort();
        out
    }

    #[test]
    fn a_project_with_the_folder_keeps_its_pages_and_its_index_in_itself() {
        let (_d, root, project, s) = with_project();
        s.write_page("tulipafin", "decisoes/adr-004.md", "# ADR 004\n").unwrap();
        s.write_index("tulipafin", "{}\n").unwrap();
        assert_eq!(files_under(&project.join("docs/hark")), vec!["decisoes/adr-004.md", "index.json"]);
        assert_eq!(s.read_page("tulipafin", "decisoes/adr-004.md").unwrap(), "# ADR 004\n");
        assert_eq!(s.read_index("tulipafin").unwrap(), "{}\n");
        assert_eq!(s.list_pages("tulipafin").unwrap(), vec!["decisoes/adr-004.md"]);
        assert_eq!(s.absolute("tulipafin", "decisoes/adr-004.md").unwrap(), project.join("docs/hark/decisoes/adr-004.md"));
        assert!(files_under(&root).is_empty(), "nothing of it went to the machine's root");
    }

    #[test]
    fn what_is_the_machines_stays_out_of_the_repository() {
        // The versions kept while editing, a dashboard's cache, the comments.
        let (_d, root, project, s) = with_project();
        s.write_page("tulipafin", "uso.dash.json", "{}").unwrap();
        s.write_version("tulipafin", "uso.dash.json", "2026-10-28T10:00:00Z", "a").unwrap();
        s.write_aside("tulipafin", "uso.dash.cache.json", "{}").unwrap();
        s.write_aside("tulipafin", "uso.comments.json", "[]").unwrap();
        assert_eq!(files_under(&project.join("docs/hark")), vec!["uso.dash.json"]);
        assert_eq!(files_under(&root).len(), 3);
        assert_eq!(s.list_versions("tulipafin", "uso.dash.json").unwrap().len(), 1);
        assert_eq!(s.read_version("tulipafin", "uso.dash.json", "2026-10-28T10:00:00Z").unwrap(), "a");
        assert_eq!(s.read_aside("tulipafin", "uso.comments.json").unwrap().as_deref(), Some("[]"));
    }

    #[test]
    fn a_project_without_the_folder_keeps_its_pages_in_the_machines_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("artifacts");
        let project = dir.path().join("code").join("horta");
        std::fs::create_dir_all(&project).unwrap();
        let s = FsArtifacts::with_projects(&root, &[("horta".into(), project.clone())]);
        s.write_page("horta", "a.md", "").unwrap();
        assert_eq!(files_under(&root), vec!["horta/a.md"]);
        assert!(files_under(&project).is_empty(), "nothing is written into a repository that did not ask");
        assert!(s.homes().is_empty());
    }

    #[test]
    fn another_space_is_untouched_by_a_project_having_the_folder() {
        let (_d, root, _project, s) = with_project();
        s.write_page("pessoal", "notas.md", "").unwrap();
        assert_eq!(files_under(&root), vec!["pessoal/notas.md"]);
    }

    #[test]
    fn a_space_that_lives_in_its_project_is_listed_with_the_ones_of_the_root() {
        let (_d, _root, _project, s) = with_project();
        s.write_page("pessoal", "notas.md", "").unwrap();
        assert_eq!(s.spaces().unwrap(), vec!["pessoal", "tulipafin"]);
        // Listed once, even when the root still has a folder of its name.
        s.write_aside("tulipafin", "x.comments.json", "[]").unwrap();
        assert_eq!(s.spaces().unwrap(), vec!["pessoal", "tulipafin"]);
    }

    #[test]
    fn the_store_says_which_spaces_live_where() {
        let (_d, _root, project, s) = with_project();
        let homes = s.homes();
        assert_eq!(homes.len(), 1);
        assert_eq!(homes[0].space, "tulipafin");
        assert_eq!(PathBuf::from(&homes[0].dir), project.join("docs/hark"));
        assert_eq!(project_home(&project), Some(project.join("docs/hark")));
        assert_eq!(s.folder("tulipafin").unwrap(), project.join("docs/hark"));
        assert!(s.folder("pessoal").unwrap().ends_with("artifacts/pessoal"));
        assert!(s.folder("../x").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_that_is_a_link_is_not_a_home_and_is_never_followed() {
        // A repository can ship `docs/hark -> ~/.ssh`, or `docs -> /`.
        let dir = tempfile::tempdir().unwrap();
        let secret = dir.path().join("secret");
        std::fs::create_dir_all(&secret).unwrap();
        std::fs::write(secret.join("id.md"), "key").unwrap();
        for linked in ["docs/hark", "docs"] {
            let project = dir.path().join(linked.replace('/', "-"));
            std::fs::create_dir_all(&project).unwrap();
            if linked == "docs" {
                std::fs::create_dir_all(secret.join("hark")).unwrap();
                std::os::unix::fs::symlink(&secret, project.join("docs")).unwrap();
            } else {
                std::fs::create_dir_all(project.join("docs")).unwrap();
                std::os::unix::fs::symlink(&secret, project.join("docs/hark")).unwrap();
            }
            assert_eq!(project_home(&project), None, "{linked}");
            let root = dir.path().join(format!("root-{}", linked.replace('/', "-")));
            let s = FsArtifacts::with_projects(&root, &[("p".into(), project.clone())]);
            assert!(s.list_pages("p").unwrap().is_empty(), "{linked}");
            s.write_page("p", "a.md", "x").unwrap();
            assert_eq!(files_under(&root), vec!["p/a.md"], "{linked}: written to the root, not through the link");
        }
        assert_eq!(files_under(&secret), vec!["id.md"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_inside_a_projects_folder_is_never_followed() {
        let (d, _root, project, s) = with_project();
        let outside = d.path().join("outside.md");
        std::fs::write(&outside, "secret").unwrap();
        std::os::unix::fs::symlink(&outside, project.join("docs/hark/link.md")).unwrap();
        assert!(s.read_page("tulipafin", "link.md").is_err());
        assert!(s.write_page("tulipafin", "link.md", "x").is_err());
        assert!(s.list_pages("tulipafin").unwrap().is_empty());
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "secret");
    }

    // ---- moving a space into its project ----

    /// A space in the machine's root with two pages, an index, a version
    /// and a comments file; and a project with no folder yet.
    fn to_move() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("artifacts");
        let project = dir.path().join("code").join("tulipafin");
        std::fs::create_dir_all(&project).unwrap();
        let s = FsArtifacts::new(&root);
        s.write_page("tulipafin", "home.md", "# Home\n").unwrap();
        s.write_page("tulipafin", "decisoes/adr-004.md", "# ADR 004\n").unwrap();
        s.write_index("tulipafin", "{\"nodes\":[]}\n").unwrap();
        s.write_version("tulipafin", "home.md", "2026-10-28T10:00:00Z", "old").unwrap();
        s.write_aside("tulipafin", "home.comments.json", "[]").unwrap();
        (dir, root, project)
    }

    #[test]
    fn moving_a_space_takes_its_pages_and_index_to_the_project_and_leaves_the_rest() {
        let (_d, root, project) = to_move();
        assert_eq!(move_to_project(&root, "tulipafin", &project).unwrap(), 2);
        assert_eq!(files_under(&project.join("docs/hark")), vec!["decisoes/adr-004.md", "home.md", "index.json"]);
        let left = files_under(&root.join("tulipafin"));
        assert_eq!(left.len(), 2, "{left:?}");
        assert!(left.contains(&"home.comments.json".to_string()));
        assert!(left.iter().any(|f| f.starts_with(".history/")));
        // Read from where they are now, with what stayed behind still theirs.
        let s = FsArtifacts::with_projects(&root, &[("tulipafin".into(), project.clone())]);
        assert_eq!(s.read_page("tulipafin", "decisoes/adr-004.md").unwrap(), "# ADR 004\n");
        assert_eq!(s.read_index("tulipafin").unwrap(), "{\"nodes\":[]}\n");
        assert_eq!(s.list_versions("tulipafin", "home.md").unwrap().len(), 1);
        assert_eq!(s.read_aside("tulipafin", "home.comments.json").unwrap().as_deref(), Some("[]"));
    }

    #[test]
    fn moving_a_space_with_no_pages_still_makes_the_folder_that_is_the_choice() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("novo");
        std::fs::create_dir_all(&project).unwrap();
        assert_eq!(move_to_project(&dir.path().join("artifacts"), "novo", &project).unwrap(), 0);
        assert_eq!(project_home(&project), Some(project.join("docs/hark")));
    }

    #[test]
    fn moving_refuses_to_write_over_a_different_page_and_moves_nothing() {
        let (_d, root, project) = to_move();
        std::fs::create_dir_all(project.join("docs/hark")).unwrap();
        std::fs::write(project.join("docs/hark/home.md"), "# Another home\n").unwrap();
        let err = move_to_project(&root, "tulipafin", &project).unwrap_err().to_string();
        assert!(err.contains("home.md"), "{err}");
        assert_eq!(files_under(&project.join("docs/hark")), vec!["home.md"]);
        assert_eq!(std::fs::read_to_string(project.join("docs/hark/home.md")).unwrap(), "# Another home\n");
        assert!(root.join("tulipafin/home.md").is_file(), "the source is still where it was");
        assert!(root.join("tulipafin/decisoes/adr-004.md").is_file());
    }

    #[test]
    fn moving_over_the_same_page_already_there_is_no_conflict() {
        // A colleague moved it first and it came with the pull.
        let (_d, root, project) = to_move();
        std::fs::create_dir_all(project.join("docs/hark")).unwrap();
        std::fs::write(project.join("docs/hark/home.md"), "# Home\n").unwrap();
        assert_eq!(move_to_project(&root, "tulipafin", &project).unwrap(), 2);
        assert!(!root.join("tulipafin/home.md").exists());
    }

    #[test]
    fn moving_refuses_when_the_project_already_has_an_index_of_its_own() {
        // Two trees cannot be made one by copying a file over the other.
        let (_d, root, project) = to_move();
        std::fs::create_dir_all(project.join("docs/hark")).unwrap();
        std::fs::write(project.join("docs/hark/index.json"), "{\"nodes\":[1]}\n").unwrap();
        assert!(move_to_project(&root, "tulipafin", &project).is_err());
        assert!(root.join("tulipafin/home.md").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn moving_never_writes_through_a_link_in_the_project() {
        let (d, root, project) = to_move();
        let elsewhere = d.path().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, project.join("docs")).unwrap();
        assert!(move_to_project(&root, "tulipafin", &project).is_err());
        assert!(files_under(&elsewhere).is_empty());
        assert!(root.join("tulipafin/home.md").is_file());
    }

    #[test]
    fn moving_needs_a_project_that_is_a_folder_and_a_space_that_is_a_name() {
        let (d, root, _project) = to_move();
        assert!(move_to_project(&root, "tulipafin", &d.path().join("gone")).is_err());
        assert!(move_to_project(&root, "../tulipafin", &d.path().join("code/tulipafin")).is_err());
    }

    #[test]
    fn removing_a_page_leaves_its_siblings() {
        let (_d, s) = store();
        s.write_page("hark", "a.md", "").unwrap();
        s.write_page("hark", "b.md", "").unwrap();
        s.remove_page("hark", "a.md").unwrap();
        assert_eq!(s.list_pages("hark").unwrap(), vec!["b.md"]);
        assert!(s.remove_page("hark", "a.md").is_err(), "gone is an error, not a shrug");
    }
}

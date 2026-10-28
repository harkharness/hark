//! Artifacts on disk: `<root>/<space>/<page>`. Thin: every path is
//! checked by the domain (`validate_path`) and the one rule only the disk
//! can enforce lives here: never through a symlink, never outside the root.

use crate::ports::ArtifactStore;
use std::path::{Path, PathBuf};

pub struct FsArtifacts {
    root: PathBuf,
}

const PAGE_EXTENSIONS: [&str; 2] = ["md", "html"];

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

impl FsArtifacts {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
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

    /// The page's absolute path, with every EXISTING component checked
    /// against being a link. Components not there yet are fine: they are
    /// about to be created as real folders.
    fn page_path(&self, space: &str, path: &str) -> anyhow::Result<PathBuf> {
        crate::domain::artifacts::validate_path(path).map_err(anyhow::Error::msg)?;
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
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| PAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
            {
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
        out.sort();
        Ok(out)
    }

    fn read_index(&self, space: &str) -> anyhow::Result<String> {
        let file = self.space_dir(space)?.join("index.json");
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
        let dir = self.space_dir(space)?;
        std::fs::create_dir_all(&dir)?;
        let file = dir.join("index.json");
        if is_symlink(&file) {
            anyhow::bail!("refused: {} is a symlink", file.display());
        }
        std::fs::write(file, json)?;
        Ok(())
    }

    fn list_pages(&self, space: &str) -> anyhow::Result<Vec<String>> {
        let dir = self.space_dir(space)?;
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
    fn removing_a_page_leaves_its_siblings() {
        let (_d, s) = store();
        s.write_page("hark", "a.md", "").unwrap();
        s.write_page("hark", "b.md", "").unwrap();
        s.remove_page("hark", "a.md").unwrap();
        assert_eq!(s.list_pages("hark").unwrap(), vec!["b.md"]);
        assert!(s.remove_page("hark", "a.md").is_err(), "gone is an error, not a shrug");
    }
}

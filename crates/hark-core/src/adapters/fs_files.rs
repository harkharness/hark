//! Project file listing for the @mention autocomplete and quick-open.
//! Walks a directory tree skipping dependency/build noise, returns paths
//! relative to the root. Read-only and local: costs nothing.

use std::path::Path;

/// Directories that never hold user-editable source of interest.
const SKIP_DIRS: &[&str] = &[
    "node_modules", "target", "dist", "build", "vendor", "__pycache__", ".venv",
];

/// List files under `root` (relative paths, `/` separated), capped.
/// Hidden directories are skipped (except nothing: `.github` counts as
/// hidden too — the viewer is for project sources, not plumbing); hidden
/// FILES like `.gitignore` at any visited level are kept.
pub fn list_files(root: &Path, cap: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut queue = std::collections::VecDeque::from([root.to_path_buf()]);
    while let Some(dir) = queue.pop_front() {
        if out.len() >= cap {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<_> = entries.map_while(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            if out.len() >= cap {
                break;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();
            if path.is_dir() {
                if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                    continue;
                }
                queue.push_back(path);
            } else if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_files_skipping_noise_dirs_and_hidden_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("src/domain")).unwrap();
        std::fs::create_dir_all(root.join("node_modules/lib")).unwrap();
        std::fs::create_dir_all(root.join(".git/objects")).unwrap();
        std::fs::write(root.join("README.md"), "x").unwrap();
        std::fs::write(root.join(".gitignore"), "x").unwrap();
        std::fs::write(root.join("src/main.rs"), "x").unwrap();
        std::fs::write(root.join("src/domain/gate.rs"), "x").unwrap();
        std::fs::write(root.join("node_modules/lib/index.js"), "x").unwrap();
        std::fs::write(root.join(".git/objects/blob"), "x").unwrap();

        let files = list_files(root, 100);
        assert!(files.contains(&"README.md".to_string()));
        assert!(files.contains(&".gitignore".to_string()));
        assert!(files.contains(&"src/main.rs".to_string()));
        assert!(files.contains(&"src/domain/gate.rs".to_string()));
        assert!(!files.iter().any(|f| f.contains("node_modules")));
        assert!(!files.iter().any(|f| f.contains(".git/")));
    }

    #[test]
    fn respects_the_cap() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..20 {
            std::fs::write(dir.path().join(format!("f{i}.txt")), "x").unwrap();
        }
        assert_eq!(list_files(dir.path(), 5).len(), 5);
    }
}

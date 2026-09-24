//! What Hark will hand to macOS `open`.
//!
//! `open` launches whatever the target's type says: an http URL opens the
//! browser, but a `.command` runs in Terminal, an `.app` bundle starts, a
//! `.fileloc` jumps to another file, and a bare `-a Foo` is read as an
//! option. Agent output is attacker-influenced — a repository can
//! prompt-inject an agent into printing any link — so a click must never
//! be able to run code: only web URLs and a short list of document types
//! go out.
//!
//! Pure. Expanding `~`, resolving symlinks and checking that the target is
//! a regular file live in the shell, which asks [`allowed_file`] again
//! about the resolved path.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opener {
    Url(String),
    File(PathBuf),
}

/// Types `open` hands to a viewer or the browser. None of them runs code
/// on open; everything else — scripts, bundles, `.command`, location
/// files — is refused, whatever handlers the user has installed.
const DOCUMENTS: &[&str] = &[
    "pdf", "png", "jpg", "jpeg", "gif", "webp", "svg", "html", "htm", "md", "markdown", "txt",
    "csv", "json",
];

/// Judged by the LAST extension: `report.pdf.command` is a command.
pub fn allowed_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| DOCUMENTS.contains(&e.to_ascii_lowercase().as_str()))
}

pub fn classify(target: &str) -> Result<Opener, String> {
    let target = target.trim();
    if target.starts_with('-') {
        return Err(format!("refused: {target} reads as an option"));
    }
    let lower = target.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Ok(Opener::Url(target.to_string()));
    }
    // Any other scheme (file:, vscode:, x-apple...:) and any relative
    // path stops here: neither starts with "/" or "~/".
    if !(target.starts_with('/') || target.starts_with("~/")) {
        return Err(format!("refused: {target} is neither a web URL nor an absolute path"));
    }
    let path = PathBuf::from(target);
    if !allowed_file(&path) {
        return Err(format!("refused: Hark opens documents outside the app, not {target}"));
    }
    Ok(Opener::File(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_urls_go_to_the_browser() {
        assert_eq!(
            classify("https://example.com/a?b=c"),
            Ok(Opener::Url("https://example.com/a?b=c".into()))
        );
        assert_eq!(classify("HTTP://example.com"), Ok(Opener::Url("HTTP://example.com".into())));
    }

    #[test]
    fn documents_open_by_absolute_or_home_path() {
        assert_eq!(
            classify("/Users/dev/report.pdf"),
            Ok(Opener::File("/Users/dev/report.pdf".into()))
        );
        assert_eq!(classify("~/notes/plan.md"), Ok(Opener::File("~/notes/plan.md".into())));
    }

    #[test]
    fn document_extensions_compare_without_case() {
        assert!(allowed_file(Path::new("/tmp/A.PDF")));
    }

    #[test]
    fn other_schemes_are_refused() {
        for t in [
            "file:///tmp/x.command",
            "javascript:alert(1)",
            "vscode://file/tmp/x",
            "ssh://host",
            "x-apple.systempreferences:com.apple.preference.security",
        ] {
            assert!(classify(t).is_err(), "{t}");
        }
    }

    #[test]
    fn a_leading_dash_is_never_an_option() {
        assert!(classify("-a/System/Applications/Calculator.app").is_err());
        assert!(classify("--args").is_err());
    }

    #[test]
    fn relative_paths_are_refused() {
        assert!(classify("docs/plan.md").is_err());
    }

    #[test]
    fn executables_bundles_and_redirects_are_refused() {
        for t in [
            "/tmp/x.command",
            "/tmp/x.terminal",
            "/tmp/x.tool",
            "/Applications/Calculator.app",
            "/tmp/x.sh",
            "/tmp/x.py",
            "/tmp/x.jar",
            "/tmp/x.webloc",
            "/tmp/x.fileloc",
            "/tmp/Makefile",
        ] {
            assert!(classify(t).is_err(), "{t}");
        }
    }

    #[test]
    fn only_the_last_extension_counts() {
        assert!(!allowed_file(Path::new("/tmp/A.COMMAND")));
        assert!(!allowed_file(Path::new("/tmp/report.pdf.command")));
    }
}

//! What differs per desktop: how a file is revealed in its file manager,
//! how a URL or document is handed to the OS, which shell a terminal
//! starts with. Each is a pure plan (tested per OS) plus a runner that
//! spawns it. Paths and targets travel as arguments, never through a
//! shell: a page title is agent output.

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmd {
    pub program: String,
    pub args: Vec<String>,
}

impl Cmd {
    fn new(program: &str, args: &[&str]) -> Self {
        Cmd {
            program: program.into(),
            args: args.iter().map(|a| a.to_string()).collect(),
        }
    }
}

/// `file://` URI for a local path, every segment percent-encoded, the
/// separators kept: what the freedesktop file-manager interface takes.
fn file_uri(path: &Path) -> String {
    let mut out = String::from("file://");
    for part in path.components() {
        match part {
            std::path::Component::RootDir => {}
            std::path::Component::Prefix(p) => out.push_str(&p.as_os_str().to_string_lossy()),
            other => {
                out.push('/');
                out.push_str(&urlencoding::encode(&other.as_os_str().to_string_lossy()));
            }
        }
    }
    out
}

/// The file manager with `path` selected. Several candidates, in order:
/// the first that runs and succeeds wins, so a desktop without the D-Bus
/// file-manager interface still gets the folder opened.
pub fn reveal_plan(path: &Path) -> Vec<Cmd> {
    let p = path.to_string_lossy();
    if cfg!(target_os = "macos") {
        vec![Cmd::new("open", &["-R", &p])]
    } else if cfg!(target_os = "windows") {
        vec![Cmd::new("explorer", &[&format!("/select,{p}")])]
    } else {
        let parent = path.parent().unwrap_or(path).to_string_lossy().into_owned();
        vec![
            Cmd::new(
                "dbus-send",
                &[
                    "--session",
                    "--print-reply",
                    "--dest=org.freedesktop.FileManager1",
                    "/org/freedesktop/FileManager1",
                    "org.freedesktop.FileManager1.ShowItems",
                    &format!("array:string:{}", file_uri(path)),
                    "string:",
                ],
            ),
            Cmd::new("xdg-open", &[&parent]),
        ]
    }
}

/// A URL or a document, opened by whatever the OS has for it.
pub fn open_plan(target: &str) -> Cmd {
    if cfg!(target_os = "macos") {
        Cmd::new("open", &[target])
    } else if cfg!(target_os = "windows") {
        // `start`'s first quoted argument is the window title; an empty
        // one keeps a quoted target from being taken as the title.
        Cmd::new("cmd", &["/C", "start", "", target])
    } else {
        Cmd::new("xdg-open", &[target])
    }
}

/// The shell a terminal pane starts: the user's own, else the desktop's.
pub fn default_shell() -> String {
    if let Some(shell) = std::env::var("SHELL").ok().filter(|s| !s.is_empty()) {
        return shell;
    }
    if cfg!(target_os = "macos") {
        "/bin/zsh".into()
    } else if cfg!(target_os = "windows") {
        "powershell.exe".into()
    } else {
        "/bin/bash".into()
    }
}

fn run(cmd: &Cmd) -> Result<(), String> {
    let status = std::process::Command::new(&cmd.program)
        .args(&cmd.args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("{}: {e}", cmd.program))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{} exited with {status}", cmd.program))
    }
}

pub fn reveal(path: &Path) -> Result<(), String> {
    let mut last = String::from("nothing to reveal with");
    for cmd in reveal_plan(path) {
        match run(&cmd) {
            Ok(()) => return Ok(()),
            Err(e) => last = e,
        }
    }
    Err(last)
}

pub fn open(target: &str) -> Result<(), String> {
    run(&open_plan(target))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_uri_encodes_each_segment_and_keeps_the_separators() {
        assert_eq!(file_uri(Path::new("/home/u/my file#1.html")), "file:///home/u/my%20file%231.html");
    }

    #[test]
    fn revealing_passes_the_path_as_one_argument_on_every_desktop() {
        let plan = reveal_plan(Path::new("/x/sem título.html"));
        assert!(!plan.is_empty());
        if cfg!(target_os = "macos") {
            assert_eq!(plan, vec![Cmd::new("open", &["-R", "/x/sem título.html"])]);
        } else if cfg!(target_os = "windows") {
            assert_eq!(plan[0].program, "explorer");
            assert_eq!(plan[0].args, vec!["/select,/x/sem título.html"]);
        } else {
            // The D-Bus interface first, with the item as a URI; then the
            // folder, for desktops without it.
            assert_eq!(plan[0].program, "dbus-send");
            assert!(plan[0].args.contains(&"array:string:file:///x/sem%20t%C3%ADtulo.html".to_string()));
            assert_eq!(plan[1], Cmd::new("xdg-open", &["/x"]));
        }
    }

    #[test]
    fn opening_hands_the_target_to_the_desktop_untouched() {
        let cmd = open_plan("https://example.com/a?b=c");
        assert_eq!(cmd.args.last().map(String::as_str), Some("https://example.com/a?b=c"));
        let expected = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "xdg-open"
        };
        assert_eq!(cmd.program, expected);
    }

    #[test]
    fn the_default_shell_is_the_desktops_when_the_environment_says_nothing() {
        // SHELL is set on any developer machine; the fallback is what a
        // bare desktop session or a service launch would see.
        let fallback = if cfg!(target_os = "macos") {
            "/bin/zsh"
        } else if cfg!(target_os = "windows") {
            "powershell.exe"
        } else {
            "/bin/bash"
        };
        let shell = default_shell();
        assert!(shell == fallback || std::env::var("SHELL").ok().as_deref() == Some(shell.as_str()));
    }
}

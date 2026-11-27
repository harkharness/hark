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

/// Where a terminal pane starts: the folder asked for when it is one,
/// else home. Never the app's own working directory: launched from the
/// desktop that is the root of the disk.
pub fn shell_dir(asked: Option<&str>, home: &Path) -> std::path::PathBuf {
    asked
        .filter(|dir| !dir.is_empty())
        .map(std::path::PathBuf::from)
        .filter(|dir| dir.is_dir())
        .unwrap_or_else(|| home.to_path_buf())
}

/// Whether a command line is an agent run without a person at it: the
/// stream mode Hark drives its workers in. A session someone resumed at a
/// terminal has neither flag.
pub fn headless_agent(command: &str) -> bool {
    let args: Vec<&str> = command.split_whitespace().collect();
    args.iter().any(|a| *a == "-p" || *a == "--print") && command.contains("--input-format stream-json")
}

/// Whether a worker of Hark's has no Hark above it any more. `parent` is
/// the parent's command line, None when the parent is gone. A dead parent
/// hands the child to pid 1 (or to the session's reaper on Linux); a live
/// Hark, this one or another running beside it, still owns its workers.
pub fn abandoned_worker(command: &str, ppid: i32, parent: Option<&str>) -> bool {
    let under_hark = ppid > 1 && parent.is_some_and(|p| p.to_lowercase().contains("hark"));
    headless_agent(command) && !under_hark
}

/// `ps -o ppid=,command=` for one process: the parent's pid, then the
/// whole command line. None when the process is gone.
fn ps_row(pid: i32) -> Option<(i32, String)> {
    let out = std::process::Command::new("ps")
        .args(["-o", "ppid=,command=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let (ppid, command) = text.trim().split_once(char::is_whitespace)?;
    Some((ppid.parse().ok()?, command.trim().to_string()))
}

/// Whether process `pid` is a worker a dead Hark left behind. None when
/// it cannot be told: the process is gone, or this desktop has no `ps`.
pub fn abandoned(pid: i32) -> Option<bool> {
    if cfg!(target_os = "windows") {
        return None;
    }
    let (ppid, command) = ps_row(pid)?;
    let parent = ps_row(ppid).map(|(_, c)| c);
    Some(abandoned_worker(&command, ppid, parent.as_deref()))
}

/// Ask a process to end, the polite way: it gets to close its files.
pub fn terminate(pid: i32) -> Result<(), String> {
    if cfg!(target_os = "windows") {
        return Err("not on this desktop".into());
    }
    run(&Cmd::new("kill", &["-TERM", &pid.to_string()]))
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

    const WORKER: &str = "/home/u/.local/bin/claude -p --input-format stream-json --resume abc --output-format stream-json";

    #[test]
    fn a_worker_whose_hark_died_is_abandoned() {
        // Handed to pid 1, or to a reaper that is not Hark.
        assert!(abandoned_worker(WORKER, 1, Some("/sbin/launchd")));
        assert!(abandoned_worker(WORKER, 812, Some("/usr/lib/systemd/systemd --user")));
        assert!(abandoned_worker(WORKER, 4000, None));
    }

    #[test]
    fn a_worker_under_a_live_hark_is_not_abandoned() {
        // Another Hark running beside this one still owns its workers.
        assert!(!abandoned_worker(WORKER, 900, Some("/Applications/Hark.app/Contents/MacOS/hark-app")));
        assert!(!abandoned_worker(WORKER, 901, Some("target/debug/hark-app")));
    }

    #[test]
    fn a_session_someone_resumed_at_a_terminal_is_never_a_worker() {
        // No stream mode: a person is typing into it, orphan or not.
        assert!(!abandoned_worker("claude --resume abc", 1, Some("/sbin/launchd")));
        assert!(!abandoned_worker("claude", 2200, Some("-zsh")));
        // A flag that merely starts with -p is not the print flag.
        assert!(!headless_agent("claude --permission-mode auto --resume abc"));
        assert!(headless_agent(WORKER));
    }

    #[test]
    fn a_terminal_starts_in_the_folder_asked_for() {
        let dir = std::env::temp_dir();
        assert_eq!(shell_dir(Some(&dir.to_string_lossy()), Path::new("/home/u")), dir);
    }

    #[test]
    fn a_terminal_with_no_folder_or_a_missing_one_starts_at_home_never_at_the_root() {
        let home = Path::new("/home/u");
        assert_eq!(shell_dir(None, home), home);
        assert_eq!(shell_dir(Some(""), home), home);
        assert_eq!(shell_dir(Some("/no/such/folder/anywhere"), home), home);
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

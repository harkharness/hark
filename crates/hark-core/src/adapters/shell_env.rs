//! Recovering the user's real PATH when no shell started us.
//!
//! Pairs with `domain::shell_path`, which decides IF and WHAT; this asks
//! the machine.

use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

/// Bracketing the answer means a profile that prints a banner, a version
/// nag or a direnv line cannot be mistaken for a PATH.
const MARK: &str = "__hark_path__";

/// How long the login shell gets. A profile with plugin managers in it
/// can take a second; anything past this is hung, and a hung startup is
/// worse than an incomplete PATH.
const BUDGET: Duration = Duration::from_secs(8);

/// The PATH the user's login shell builds. `-l` sources the login
/// profile (`.zprofile`, `.bash_profile`) — where Homebrew, asdf, nvm and
/// friends install themselves — without the cost and the hazards of an
/// interactive shell.
pub fn login_path() -> Option<String> {
    let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty())?;
    let script = format!("printf '{MARK}%s{MARK}' \"$PATH\"");
    let mut child = std::process::Command::new(shell)
        .args(["-l", "-c", &script])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let mut out = child.stdout.take()?;

    // Command has no timeout of its own, and a profile that blocks would
    // otherwise hold the window closed forever.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = out.read_to_string(&mut buf);
        let _ = tx.send(buf);
    });
    let text = match rx.recv_timeout(BUDGET) {
        Ok(text) => text,
        Err(_) => {
            let _ = child.kill();
            return None;
        }
    };
    let _ = child.wait();

    let (_, rest) = text.split_once(MARK)?;
    let (path, _) = rest.split_once(MARK)?;
    (!path.trim().is_empty()).then(|| path.to_string())
}

/// Where user-installed tools live on macOS, when the shell could not be
/// asked. The same reasoning as `Config::claude_bin_resolved`, widened
/// from one binary to the whole environment.
pub fn well_known_bins() -> Vec<String> {
    let home = PathBuf::from(crate::config::expand_home("~"));
    let candidates: [PathBuf; 5] = [
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        home.join(".local").join("bin"),
        home.join(".cargo").join("bin"),
        home.join("bin"),
    ];
    candidates
        .into_iter()
        .filter(|p| p.is_dir())
        .map(|p| p.display().to_string())
        .collect()
}

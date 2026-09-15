//! The shell side of the version check: fetch `registry.json` (curl, as
//! the model download does) and ask a binary its `--version`. Both are
//! bounded in time — a catalog that hangs on a slow network or a
//! misbehaving binary is worse than one that says "unknown".

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Where the ACP registry publishes what is current.
pub const REGISTRY_URL: &str = "https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json";

/// The registry's platform key for this machine ("darwin-aarch64").
pub fn platform_key() -> String {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };
    format!("{os}-{}", std::env::consts::ARCH)
}

/// Download the registry document. Fails loudly and quickly.
pub fn fetch_registry(url: &str) -> anyhow::Result<String> {
    let out = Command::new("curl")
        .args(["-fsSL", "--max-time", "15", url])
        .stdin(Stdio::null())
        .output()?;
    if !out.status.success() {
        anyhow::bail!("curl: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Run `cmd args…` and return its first line of output, or None when it
/// is not there, fails, says nothing, or takes longer than `budget`.
pub fn probe(cmd: &str, args: &[&str], budget: Duration) -> Option<String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < budget => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let mut text = String::new();
    child.stdout.take()?.read_to_string(&mut text).ok()?;
    if text.trim().is_empty() {
        // Some binaries print their version on stderr.
        child.stderr.take()?.read_to_string(&mut text).ok()?;
    }
    let first = text.lines().find(|l| !l.trim().is_empty())?.trim().to_string();
    (!first.is_empty()).then_some(first)
}

/// `cmd --version`, read as a version number.
pub fn installed_version(cmd: &str) -> Option<String> {
    probe(cmd, &["--version"], Duration::from_secs(8))
        .and_then(|line| crate::domain::registry::installed_version(&line))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_probe_reads_the_first_line_and_gives_up_on_a_hang() {
        assert_eq!(
            probe("sh", &["-c", "echo tool 9.9.9; echo more"], Duration::from_secs(5)).as_deref(),
            Some("tool 9.9.9")
        );
        assert_eq!(probe("sh", &["-c", "sleep 5"], Duration::from_millis(200)), None);
        assert_eq!(probe("hark-no-such-binary-xyz", &["--version"], Duration::from_secs(1)), None);
    }

    #[test]
    fn a_version_probe_reads_the_number_out_of_the_words() {
        // The real shape of `codex-acp --version`, via a stand-in.
        let cmd = std::env::temp_dir().join("hark-fake-version-probe.sh");
        std::fs::write(&cmd, "#!/bin/sh\necho '@agentclientprotocol/codex-acp 1.11.0'\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&cmd, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(installed_version(cmd.to_str().unwrap()).as_deref(), Some("1.11.0"));
    }

    #[test]
    fn the_platform_key_is_the_registrys_spelling() {
        let key = platform_key();
        assert!(key.starts_with("darwin-") || key.starts_with("linux-"), "{key}");
        assert!(!key.contains("macos"));
    }
}

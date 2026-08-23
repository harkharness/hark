//! Opt-in bridge to Claude Code's `statusLine` hook.
//!
//! The subscription percentages (5-hour window, weekly, per-model weekly)
//! exist in exactly one place: the JSON the CLI pipes into the statusLine
//! command. This adapter installs a wrapper script that tees that stdin
//! into a file Hark reads, then runs whatever command was configured before,
//! so the user's own status line keeps working.
//!
//! Rules: nothing here runs without an explicit user click; the previous
//! `~/.claude/settings.json` is backed up before the first write; and
//! uninstall restores exactly what was there.

use crate::statusline::StatusLine;
use std::path::{Path, PathBuf};

pub struct BridgePaths {
    /// `~/.claude/settings.json`.
    pub settings: PathBuf,
    /// Where the wrapper script is written (Hark data dir).
    pub script: PathBuf,
    /// Where the payload lands (Hark data dir).
    pub payload: PathBuf,
    /// Copy of the settings file made before the first install.
    pub backup: PathBuf,
}

impl BridgePaths {
    pub fn new(home: &Path, data_dir: &Path) -> Self {
        Self {
            settings: home.join(".claude").join("settings.json"),
            script: data_dir.join("statusline-bridge.sh"),
            payload: data_dir.join("statusline.json"),
            backup: data_dir.join("settings.json.before-hark"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BridgeStatus {
    pub installed: bool,
    /// Seconds since the payload was last written (None = never).
    pub age_secs: Option<u64>,
    pub payload_path: String,
}

fn read_settings(path: &Path) -> serde_json::Map<String, serde_json::Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

fn current_command(settings: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    settings
        .get("statusLine")?
        .get("command")?
        .as_str()
        .map(str::to_string)
}

/// The status line the USER configured (our own wrapper doesn't count).
fn user_command(settings: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    current_command(settings).filter(|cmd| !cmd.contains("statusline-bridge.sh"))
}

/// Is our own wrapper the configured status line?
pub fn is_installed(paths: &BridgePaths) -> bool {
    current_command(&read_settings(&paths.settings))
        .map(|cmd| cmd.contains("statusline-bridge.sh"))
        .unwrap_or(false)
}

pub fn status(paths: &BridgePaths) -> BridgeStatus {
    let age_secs = std::fs::metadata(&paths.payload)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .map(|d| d.as_secs());
    BridgeStatus {
        installed: is_installed(paths),
        age_secs,
        payload_path: paths.payload.display().to_string(),
    }
}

/// The wrapper: capture stdin, write it atomically, then feed the very same
/// bytes to the original command (if there was one) so its output still
/// reaches the status line.
fn script_body(payload: &Path, original: Option<&str>) -> String {
    let payload = payload.display();
    let forward = match original {
        Some(cmd) => format!("printf '%s' \"$payload\" | {cmd}\n"),
        None => String::new(),
    };
    format!(
        "#!/bin/sh\n\
         # Written by Hark (statusLine bridge). Delete freely: Hark only reads\n\
         # the file below and falls back to nothing when it is missing.\n\
         payload=$(cat)\n\
         printf '%s' \"$payload\" > '{payload}.tmp' 2>/dev/null && mv '{payload}.tmp' '{payload}'\n\
         {forward}"
    )
}

/// Install the bridge. Returns the command that was replaced, if any.
pub fn install(paths: &BridgePaths) -> anyhow::Result<Option<String>> {
    if let Some(parent) = paths.script.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut settings = read_settings(&paths.settings);
    // Never wrap ourselves twice: on a reinstall the user's own command is
    // no longer in settings (we replaced it), so recover it from the backup.
    let original =
        user_command(&settings).or_else(|| user_command(&read_settings(&paths.backup)));

    if paths.settings.exists() && !paths.backup.exists() {
        std::fs::copy(&paths.settings, &paths.backup)?;
    }
    std::fs::write(&paths.script, script_body(&paths.payload, original.as_deref()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&paths.script, std::fs::Permissions::from_mode(0o755))?;
    }

    settings.insert(
        "statusLine".into(),
        serde_json::json!({
            "type": "command",
            "command": format!("sh {}", paths.script.display()),
        }),
    );
    if let Some(parent) = paths.settings.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &paths.settings,
        format!("{:#}\n", serde_json::Value::Object(settings)),
    )?;
    Ok(original)
}

/// Put the previous status line back (from the backup when we have one).
pub fn uninstall(paths: &BridgePaths) -> anyhow::Result<()> {
    if !is_installed(paths) {
        return Ok(());
    }
    if paths.backup.exists() {
        std::fs::copy(&paths.backup, &paths.settings)?;
        let _ = std::fs::remove_file(&paths.backup);
    } else {
        let mut settings = read_settings(&paths.settings);
        settings.remove("statusLine");
        std::fs::write(
            &paths.settings,
            format!("{:#}\n", serde_json::Value::Object(settings)),
        )?;
    }
    let _ = std::fs::remove_file(&paths.script);
    Ok(())
}

/// Stale data is worse than no data: a status line that stopped rendering
/// an hour ago would still look current on screen.
fn fresh_enough(age_secs: Option<u64>, max_age_secs: u64) -> bool {
    age_secs.map(|age| age <= max_age_secs).unwrap_or(false)
}

/// Read the last payload, or None when it is missing or too old.
pub fn read(paths: &BridgePaths, max_age_secs: u64) -> Option<StatusLine> {
    if !fresh_enough(status(paths).age_secs, max_age_secs) {
        return None;
    }
    let text = std::fs::read_to_string(&paths.payload).ok()?;
    crate::statusline::parse(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hark-bridge-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn install_backs_up_wraps_the_previous_command_and_uninstall_restores() {
        let root = temp_dir("roundtrip");
        let home = root.join("home");
        let data = root.join("data");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        let paths = BridgePaths::new(&home, &data);
        let before = r#"{"statusLine":{"type":"command","command":"bash ~/mine.sh"},"other":1}"#;
        std::fs::write(&paths.settings, before).unwrap();

        assert!(!is_installed(&paths));
        let replaced = install(&paths).unwrap();
        assert_eq!(replaced.as_deref(), Some("bash ~/mine.sh"));
        assert!(is_installed(&paths));
        assert!(paths.backup.exists(), "the user's settings must be recoverable");

        // The wrapper keeps the user's own status line alive.
        let script = std::fs::read_to_string(&paths.script).unwrap();
        assert!(script.contains("bash ~/mine.sh"), "original command forwarded");
        assert!(script.contains("statusline.json"), "payload teed to disk");
        // Unrelated settings survive.
        let after = std::fs::read_to_string(&paths.settings).unwrap();
        assert!(after.contains("\"other\""));

        // Installing twice must not wrap the wrapper.
        assert_eq!(install(&paths).unwrap().as_deref(), Some("bash ~/mine.sh"));

        uninstall(&paths).unwrap();
        assert!(!is_installed(&paths));
        assert_eq!(
            std::fs::read_to_string(&paths.settings).unwrap(),
            before,
            "uninstall restores the file byte for byte"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reads_fresh_payloads_and_refuses_stale_ones() {
        let root = temp_dir("read");
        let data = root.join("data");
        std::fs::create_dir_all(&data).unwrap();
        let paths = BridgePaths::new(&root.join("home"), &data);

        // Nothing written yet.
        assert!(read(&paths, 600).is_none());
        assert_eq!(status(&paths).age_secs, None);

        std::fs::write(
            &paths.payload,
            r#"{"rate_limits":{"five_hour":{"used_percentage":64}}}"#,
        )
        .unwrap();
        let line = read(&paths, 600).expect("just written = fresh");
        assert!((line.limits[0].used - 0.64).abs() < 1e-9);
        assert!(status(&paths).age_secs.is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn stale_or_missing_payloads_show_nothing() {
        assert!(fresh_enough(Some(0), 600));
        assert!(fresh_enough(Some(600), 600));
        assert!(!fresh_enough(Some(601), 600), "older than the window = stale");
        assert!(!fresh_enough(None, 600), "never written = nothing to show");
    }
}

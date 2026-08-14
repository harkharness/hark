//! User configuration (`~/.config/vox/config.toml`) with sane defaults.
//! Everything is overridable; nothing is hardcoded to any specific machine.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Claude binary name or absolute path.
    pub claude_bin: String,
    /// Model alias passed to `claude --model`.
    pub model: String,
    /// Where Claude Code keeps session logs.
    pub projects_dir: PathBuf,
    /// Repositories included in the context snapshot.
    pub repos: Vec<String>,
    /// How far back "recent sessions" reaches.
    pub hours_back: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            claude_bin: "claude".into(),
            model: "sonnet".into(),
            projects_dir: home().join(".claude").join("projects"),
            repos: Vec::new(),
            hours_back: 36,
        }
    }
}

impl Config {
    /// Load from the default path, falling back to defaults when absent.
    pub fn load() -> Self {
        let path = home().join(".config").join("vox").join("config.toml");
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Local state directory (index database, whisper models later).
    pub fn data_dir(&self) -> PathBuf {
        let dir = if cfg!(target_os = "macos") {
            home().join("Library").join("Application Support").join("vox")
        } else {
            home().join(".local").join("share").join("vox")
        };
        let _ = std::fs::create_dir_all(&dir);
        dir
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

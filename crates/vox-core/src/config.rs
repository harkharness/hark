//! User configuration (`~/.config/vox/config.toml`) with sane defaults.
//! Everything is overridable; nothing is hardcoded to any specific machine.

use crate::domain::context::ContextDef;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ContextTable {
    /// cwd prefixes that put a session inside this context (`~` allowed).
    pub match_cwd: Vec<String>,
    /// Repositories collected into the snapshot for this context.
    pub repos: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Claude binary name or absolute path.
    pub claude_bin: String,
    /// Model alias passed to `claude --model`.
    pub model: String,
    /// Where Claude Code keeps session logs.
    pub projects_dir: PathBuf,
    /// Repositories for the "all" context (no named context active).
    pub repos: Vec<String>,
    /// How far back "recent sessions" reaches.
    pub hours_back: i64,
    /// Context active when none was chosen; "all" means no filter.
    pub default_context: String,
    /// Named focus areas, kubectl-context style.
    pub contexts: BTreeMap<String, ContextTable>,
    /// STT language (whisper).
    pub language: String,
    /// macOS `say` voice for answers.
    pub voice: String,
    /// Whisper ggml model path; empty means `<data_dir>/models/ggml-small.bin`.
    pub whisper_model: String,
    /// Vocabulary bias fed to whisper (helps tech terms inside pt-BR speech).
    pub vocab: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            claude_bin: "claude".into(),
            model: "sonnet".into(),
            projects_dir: home().join(".claude").join("projects"),
            repos: Vec::new(),
            hours_back: 36,
            default_context: "all".into(),
            contexts: BTreeMap::new(),
            language: "pt".into(),
            voice: "Luciana".into(),
            whisper_model: String::new(),
            vocab: [
                "webhook", "pull request", "PR", "deploy", "Claude Code", "branch", "commit",
                "migração", "cluster", "vox",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
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

    /// Context names as declared, for hints and validation.
    pub fn context_names(&self) -> Vec<String> {
        self.contexts.keys().cloned().collect()
    }

    /// Resolve a context by name with `~` expanded. "all" and unknown names
    /// resolve to None (no filter).
    pub fn context(&self, name: &str) -> Option<ContextDef> {
        self.contexts.get(name).map(|t| ContextDef {
            name: name.to_string(),
            match_cwd: t.match_cwd.iter().map(|p| expand_home(p)).collect(),
            repos: t.repos.iter().map(|p| expand_home(p)).collect(),
        })
    }

    /// Resolved whisper model path (config override or data-dir default).
    pub fn whisper_model_path(&self) -> PathBuf {
        if self.whisper_model.is_empty() {
            self.data_dir().join("models").join("ggml-small.bin")
        } else {
            PathBuf::from(expand_home(&self.whisper_model))
        }
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

fn expand_home(path: &str) -> String {
    path.strip_prefix("~")
        .map(|rest| format!("{}{rest}", home().display()))
        .unwrap_or_else(|| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_contexts_from_toml() {
        let toml_text = r#"
default_context = "alpha"
[contexts.alpha]
match_cwd = ["~/Projects/alpha"]
repos = ["~/Projects/alpha"]
[contexts.beta]
match_cwd = ["/abs/beta"]
"#;
        let config: Config = toml::from_str(toml_text).unwrap();
        assert_eq!(config.default_context, "alpha");
        assert_eq!(config.context_names(), vec!["alpha", "beta"]);

        let alpha = config.context("alpha").unwrap();
        assert!(!alpha.match_cwd[0].starts_with('~'), "home must be expanded");
        assert!(alpha.match_cwd[0].ends_with("/Projects/alpha"));
        assert!(config.context("all").is_none());
        assert!(config.context("nope").is_none());
    }
}

//! User configuration (`~/.config/hark/config.toml`) with sane defaults.
//! Everything is overridable; nothing is hardcoded to any specific machine.

use crate::domain::context::ContextDef;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, serde::Serialize, Default)]
#[serde(default)]
pub struct ContextTable {
    /// cwd prefixes that put a session inside this context (`~` allowed).
    pub match_cwd: Vec<String>,
    /// Repositories collected into the snapshot for this context.
    pub repos: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
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
    /// STT language (whisper) — what the mic EXPECTS TO HEAR.
    pub language: String,
    /// UI language ("pt" | "en") — what the SCREEN shows. Separate on
    /// purpose: plenty of people speak pt-BR to an English interface.
    pub ui_language: String,
    /// macOS `say` voice for answers.
    pub voice: String,
    /// Whisper ggml model path; empty means `<data_dir>/models/ggml-small.bin`.
    pub whisper_model: String,
    /// Vocabulary bias fed to whisper (helps tech terms inside pt-BR speech).
    pub vocab: Vec<String>,
    /// Model tiers for the router (light/standard/heavy/max).
    pub models: ModelsTable,
    /// Color scheme for code surfaces (chat blocks, editor, terminal).
    /// Built-in: "hark" (default) and "dracula".
    pub theme: String,
    /// Character budget of the ask prompt (~chars/4 tokens). Default keeps
    /// a question around 3k input tokens.
    pub prompt_budget_chars: usize,
    /// Global hotkey that opens the mic from anywhere (mother window).
    pub hotkey: String,
    /// Dollar ceiling per worker process (`--max-budget-usd`). The
    /// post-incident guardrail: 0 disables. Default 2.0.
    pub worker_budget_usd: f64,
    /// Optional turn ceiling per worker process (`--max-turns`); 0 = off.
    pub worker_max_turns: u32,
    /// Default permission mode of new workers when the instruction names
    /// none ("manual" | "acceptEdits" | "plan" | "auto" | "bypass").
    /// Empty = the CLI's own default (ask for everything).
    pub worker_mode: String,
    /// The assistant's own name (persona of the mother's work chat, UI
    /// labels, spoken announcements).
    pub assistant_name: String,
    /// Community eco-tools, per WORKER process (env vars — the global
    /// settings are never touched). Empty = evidence-based defaults:
    /// ponytail "full" when installed, caveman "off", tokensave "off".
    pub assist: AssistTable,
    /// Queue follow-up messages while a worker turn is in flight and
    /// deliver them as ONE message when it ends (fewer, fatter turns).
    /// Off by default; directive changes are not applied to queued text.
    pub batch_messages: bool,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize, Default)]
#[serde(default)]
pub struct AssistTable {
    pub ponytail: Option<String>,
    pub caveman: Option<String>,
    pub tokensave: Option<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize, Default)]
#[serde(default)]
pub struct ModelsTable {
    pub light: Option<String>,
    pub standard: Option<String>,
    pub heavy: Option<String>,
    pub max: Option<String>,
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
            ui_language: "pt".into(),
            voice: "Luciana".into(),
            whisper_model: String::new(),
            vocab: [
                "webhook", "pull request", "PR", "deploy", "Claude Code", "branch", "commit",
                "migração", "cluster", "hark", "vox",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            models: ModelsTable::default(),
            theme: "hark".into(),
            prompt_budget_chars: 12_000,
            hotkey: "cmd+shift+space".into(),
            worker_budget_usd: 2.0,
            worker_max_turns: 0,
            worker_mode: String::new(),
            assistant_name: "Hark".into(),
            assist: AssistTable::default(),
            batch_messages: false,
        }
    }
}

impl Config {
    /// Configured default permission mode (None = CLI default).
    pub fn default_worker_mode(&self) -> Option<crate::domain::directives::Mode> {
        crate::domain::directives::Mode::from_flag(&self.worker_mode)
    }

    /// Hard caps applied to every worker spawn.
    pub fn spawn_limits(&self) -> hark_agent::SpawnLimits {
        hark_agent::SpawnLimits {
            max_budget_usd: (self.worker_budget_usd > 0.0).then_some(self.worker_budget_usd),
            max_turns: (self.worker_max_turns > 0).then_some(self.worker_max_turns),
        }
    }
}

/// The user config file. Single source for the three call sites that used
/// to assemble this path by hand.
pub fn config_path() -> PathBuf {
    home().join(".config").join("hark").join("config.toml")
}

/// Move a legacy directory into its new home. An empty leftover at the new
/// path (created by an early `data_dir()` call) is replaced; a populated one
/// wins and nothing moves. Returns true when a migration happened.
pub fn migrate_dir(old: &Path, new: &Path) -> bool {
    if !old.is_dir() || old == new {
        return false;
    }
    if new.exists() {
        match std::fs::read_dir(new) {
            Ok(mut entries) => {
                if entries.next().is_some() {
                    return false;
                }
                let _ = std::fs::remove_dir(new);
            }
            Err(_) => return false,
        }
    }
    if let Some(parent) = new.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::rename(old, new).is_ok()
}

/// Absolute paths written while the product was still named "vox" go stale
/// after the dir migration; patch them in place (config.toml also carries
/// the old theme name and config-dir prefix).
pub fn rewrite_legacy_paths(file: &Path, old_data: &Path, new_data: &Path) {
    let Ok(text) = std::fs::read_to_string(file) else { return };
    let mut out = text.replace(
        &old_data.display().to_string(),
        &new_data.display().to_string(),
    );
    if file.file_name().is_some_and(|n| n == "config.toml") {
        out = out
            .replace("/.config/vox", "/.config/hark")
            .replace("theme = \"vox\"", "theme = \"hark\"");
    }
    if out != text {
        let _ = std::fs::write(file, out);
    }
}

/// One-time migration from the old product name ("vox"): config dir, data
/// dir, the statusline-bridge backup, and stale absolute paths inside user
/// files. Cheap no-op on every later boot — call it before anything reads
/// config or data.
pub fn migrate_legacy() {
    migrate_dir(&home().join(".config").join("vox"), &home().join(".config").join("hark"));

    let (old_data, new_data) = if cfg!(target_os = "macos") {
        (
            home().join("Library").join("Application Support").join("vox"),
            home().join("Library").join("Application Support").join("hark"),
        )
    } else {
        (
            home().join(".local").join("share").join("vox"),
            home().join(".local").join("share").join("hark"),
        )
    };
    migrate_dir(&old_data, &new_data);

    let old_backup = new_data.join("settings.json.before-vox");
    if old_backup.exists() {
        let _ = std::fs::rename(&old_backup, new_data.join("settings.json.before-hark"));
    }

    rewrite_legacy_paths(&config_path(), &old_data, &new_data);
    rewrite_legacy_paths(&home().join(".claude").join("settings.json"), &old_data, &new_data);
}

impl Config {
    /// Load from the default path, falling back to defaults when absent.
    pub fn load() -> Self {
        std::fs::read_to_string(config_path())
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

    /// Router tiers with config overrides. `model` (legacy field) overrides
    /// the standard tier so existing configs keep working.
    pub fn models(&self) -> crate::domain::intent::Models {
        let defaults = crate::domain::intent::Models::default();
        crate::domain::intent::Models {
            light: self.models.light.clone().unwrap_or(defaults.light),
            standard: self
                .models
                .standard
                .clone()
                .unwrap_or_else(|| self.model.clone()),
            heavy: self.models.heavy.clone().unwrap_or(defaults.heavy),
            max: self.models.max.clone().unwrap_or(defaults.max),
        }
    }

    /// Resolve the Claude binary. A bare "claude" is dangerous under GUI
    /// PATHs (an old npm-installed claude may shadow the real one), so it
    /// resolves through well-known install locations first.
    pub fn claude_bin_resolved(&self) -> String {
        if self.claude_bin != "claude" {
            return expand_home(&self.claude_bin);
        }
        let known = [
            home().join(".local").join("bin").join("claude"),
            PathBuf::from("/opt/homebrew/bin/claude"),
            PathBuf::from("/usr/local/bin/claude"),
        ];
        known
            .iter()
            .find(|p| p.exists())
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "claude".to_string())
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
            home().join("Library").join("Application Support").join("hark")
        } else {
            home().join(".local").join("share").join("hark")
        };
        let _ = std::fs::create_dir_all(&dir);
        dir
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

/// Expand a leading `~` to the user's home directory.
pub fn expand_home(path: &str) -> String {
    path.strip_prefix("~")
        .map(|rest| format!("{}{rest}", home().display()))
        .unwrap_or_else(|| path.to_string())
}

/// Apply a flat {key: value} patch to the user's TOML text, preserving
/// comments, ordering and keys the UI does not know about. Dotted keys
/// ("models.light") address nested tables, creating them when missing.
/// The settings UI writes THROUGH this — the file stays the user's own.
pub fn patch_toml(text: &str, patch: &serde_json::Value) -> anyhow::Result<String> {
    use toml_edit::{value, DocumentMut, Item, Table};
    let mut doc: DocumentMut = text.parse()?;
    let entries = patch
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("patch must be an object"))?;
    for (key, val) in entries {
        let item = match val {
            serde_json::Value::String(s) => value(s.as_str()),
            serde_json::Value::Bool(b) => value(*b),
            serde_json::Value::Number(n) if n.is_i64() => value(n.as_i64().unwrap()),
            serde_json::Value::Number(n) => value(n.as_f64().unwrap_or_default()),
            other => anyhow::bail!("unsupported patch value for {key}: {other}"),
        };
        let mut parts = key.split('.').peekable();
        let mut node: &mut Item = doc.as_item_mut();
        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                node[part] = item;
                break;
            }
            if node.get(part).is_none() {
                node[part] = Item::Table(Table::new());
            }
            node = &mut node[part];
        }
    }
    Ok(doc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_preserves_comments_and_unknown_keys() {
        let original = "# my precious comment\nmodel = \"sonnet\"\nmystery = true\n";
        let out = patch_toml(
            original,
            &serde_json::json!({ "model": "opus", "voice": "Luciana" }),
        )
        .unwrap();
        assert!(out.contains("# my precious comment"), "comment survives: {out}");
        assert!(out.contains("mystery = true"), "unknown key survives");
        assert!(out.contains("model = \"opus\""));
        assert!(out.contains("voice = \"Luciana\""));
        // The result must still parse as a valid Config.
        let parsed: Config = toml::from_str(&out).unwrap();
        assert_eq!(parsed.model, "opus");
        assert_eq!(parsed.voice, "Luciana");
    }

    #[test]
    fn patch_writes_numbers_with_their_toml_types() {
        let out = patch_toml(
            "",
            &serde_json::json!({ "worker_budget_usd": 2.5, "worker_max_turns": 12 }),
        )
        .unwrap();
        assert!(out.contains("worker_budget_usd = 2.5"), "float stays float: {out}");
        assert!(out.contains("worker_max_turns = 12"), "int stays int: {out}");
        let parsed: Config = toml::from_str(&out).unwrap();
        assert_eq!(parsed.worker_budget_usd, 2.5);
        assert_eq!(parsed.worker_max_turns, 12);
    }

    #[test]
    fn patch_reaches_nested_tables_by_dotted_key() {
        let out = patch_toml(
            "model = \"sonnet\"\n",
            &serde_json::json!({ "models.light": "haiku" }),
        )
        .unwrap();
        let parsed: Config = toml::from_str(&out).unwrap();
        assert_eq!(parsed.models.light.as_deref(), Some("haiku"));
    }

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
        assert_eq!(config.theme, "hark", "theme defaults to the app scheme");

        let alpha = config.context("alpha").unwrap();
        assert!(!alpha.match_cwd[0].starts_with('~'), "home must be expanded");
        assert!(alpha.match_cwd[0].ends_with("/Projects/alpha"));
        assert!(config.context("all").is_none());
        assert!(config.context("nope").is_none());
    }

    #[test]
    fn migrate_dir_moves_the_old_name_into_the_new_one() {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join("vox");
        let new = root.path().join("hark");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("index.db"), "data").unwrap();

        assert!(migrate_dir(&old, &new));
        assert!(!old.exists());
        assert_eq!(std::fs::read_to_string(new.join("index.db")).unwrap(), "data");
    }

    #[test]
    fn migrate_dir_replaces_an_empty_leftover_but_never_a_populated_one() {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join("vox");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("index.db"), "old").unwrap();

        // Empty new dir (created by an early data_dir() call) is replaced.
        let new = root.path().join("hark");
        std::fs::create_dir_all(&new).unwrap();
        assert!(migrate_dir(&old, &new));
        assert!(new.join("index.db").exists());

        // A populated new dir wins; nothing moves.
        let old2 = root.path().join("vox2");
        std::fs::create_dir_all(&old2).unwrap();
        std::fs::write(new.join("state.json"), "{}").unwrap();
        assert!(!migrate_dir(&old2, &new));
        assert!(old2.exists());

        // Missing old dir is a no-op.
        assert!(!migrate_dir(&root.path().join("ghost"), &new));
    }

    #[test]
    fn rewrite_legacy_paths_updates_config_content() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("config.toml");
        std::fs::write(
            &file,
            "theme = \"vox\"\nwhisper_model = \"/Users/u/Library/Application Support/vox/models/ggml-small.bin\"\n",
        )
        .unwrap();
        rewrite_legacy_paths(
            &file,
            Path::new("/Users/u/Library/Application Support/vox"),
            Path::new("/Users/u/Library/Application Support/hark"),
        );
        let out = std::fs::read_to_string(&file).unwrap();
        assert!(out.contains("Application Support/hark/models"), "{out}");
        assert!(out.contains("theme = \"hark\""), "{out}");
    }
}

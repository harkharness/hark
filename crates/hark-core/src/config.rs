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
    /// STT language (whisper) — what the mic EXPECTS TO HEAR. "auto"
    /// detects per utterance (for people who mix languages mid-sentence);
    /// a fixed code is more accurate on short ones. The spoken COMMAND
    /// grammar understands Portuguese and English either way.
    pub language: String,
    /// UI language ("pt" | "en") — what the SCREEN shows. Separate on
    /// purpose: plenty of people speak pt-BR to an English interface.
    pub ui_language: String,
    /// macOS `say` voice for answers.
    pub voice: String,
    /// Whisper ggml model path; empty means `<data_dir>/models/ggml-small.bin`.
    pub whisper_model: String,
    /// Speech engine: "whisper" (local, default) or "external" — the user
    /// dictates with their own tool (Wispr/Superwhisper/OS dictation) into
    /// the HUD's text field, and Hark keeps what it is actually good at:
    /// deciding where the sentence goes. STT is a commodity (FASE 8.3).
    #[serde(default = "default_stt")]
    pub stt: String,
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
    /// Per-project budget overrides: workspace path (~/ ok) → USD ceiling.
    /// A path prefix wins over the global `worker_budget_usd`; 0 disables
    /// the cap for that project. TOML: `[project_budgets]` table.
    #[serde(default)]
    pub project_budgets: BTreeMap<String, f64>,
    /// Default permission mode of new workers when the instruction names
    /// none ("manual" | "acceptEdits" | "plan" | "auto" | "bypass").
    /// Empty = the CLI's own default (ask for everything).
    pub worker_mode: String,
    /// Default model of new workers — the composer's model pill, kept
    /// across restarts. Empty hands the choice to the router.
    pub worker_model: String,
    /// Default reasoning effort of new workers — the composer's effort
    /// pill. Empty passes no `--effort` at all, which is not "low": it
    /// leaves the CLI's own default in force.
    pub worker_effort: String,
    /// The assistant's own name (persona of the mother's work chat, UI
    /// labels, spoken announcements).
    pub assistant_name: String,
    /// Community eco-tools, per WORKER process (env vars — the global
    /// settings are never touched). Empty = evidence-based defaults:
    /// ponytail "full" when installed, caveman "off", tokensave "off".
    pub assist: AssistTable,
    /// Which agent plugin drives the sessions. Only "claude" ships today;
    /// the field exists so a second backend is a config change, not a fork.
    pub agent: AgentTable,
    /// Per-agent overrides, keyed by registry id. Built-ins (claude,
    /// gemini, claude-acp) need no entry — a table here overrides one
    /// field at a time, or declares a backend Hark never heard of.
    pub agents: std::collections::BTreeMap<String, crate::domain::agents::AgentEntry>,
    /// Check the public releases repo for a newer build (a GET against
    /// github.com — the ONLY network call Hark makes on its own; nothing
    /// identifying is sent). Updates never install themselves: the app
    /// downloads, verifies the signature, and waits for the user's click.
    pub auto_update: bool,
    /// Queue follow-up messages while a worker turn is in flight and
    /// deliver them as ONE message when it ends (fewer, fatter turns).
    /// Off by default; directive changes are not applied to queued text.
    pub batch_messages: bool,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize, Default)]
#[serde(default)]
pub struct AgentTable {
    /// Plugin id ("claude"); empty falls back to the default backend.
    pub plugin: String,
    /// Backend for the cheap ask/gate lane; empty follows `plugin`. Set it
    /// when the agent you work with is not the one you want answering
    /// "quanto gastei hoje?".
    pub ask: String,
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

fn default_stt() -> String {
    "whisper".into()
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
            agents: BTreeMap::new(),
            language: "pt".into(),
            ui_language: "pt".into(),
            voice: "Luciana".into(),
            whisper_model: String::new(),
            stt: default_stt(),
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
            project_budgets: BTreeMap::new(),
            worker_mode: String::new(),
            worker_model: String::new(),
            worker_effort: String::new(),
            assistant_name: "Hark".into(),
            agent: AgentTable::default(),
            assist: AssistTable::default(),
            auto_update: true,
            batch_messages: false,
        }
    }
}

impl Config {
    /// The language Hark writes and speaks in. Recognition accepts both
    /// languages regardless; this only picks the output side.
    pub fn lang(&self) -> crate::domain::lang::Lang {
        crate::domain::lang::Lang::from_code(&self.ui_language)
    }

    /// Language code for the speech model. "auto" (or empty) makes it
    /// detect per utterance, which is what mixing languages needs.
    pub fn stt_language(&self) -> String {
        crate::domain::lang::stt_code(&self.language)
    }

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

    /// Spawn limits for a worker in `workspace`: the project's own ceiling
    /// when one is configured (longest matching path prefix wins; 0 turns
    /// the cap off for that project), else the global one.
    pub fn spawn_limits_for(&self, workspace: &std::path::Path) -> hark_agent::SpawnLimits {
        let ws = workspace.to_string_lossy();
        let hit = self
            .project_budgets
            .iter()
            .map(|(path, usd)| (expand_home(path), usd))
            .filter(|(p, _)| *ws == **p || ws.starts_with(&format!("{p}/")))
            .max_by_key(|(p, _)| p.len());
        match hit {
            Some((_, usd)) => hark_agent::SpawnLimits {
                max_budget_usd: (*usd > 0.0).then_some(*usd),
                max_turns: (self.worker_max_turns > 0).then_some(self.worker_max_turns),
            },
            None => self.spawn_limits(),
        }
    }
}

/// The user config file. Single source for the three call sites that used
/// to assemble this path by hand.
pub fn config_path() -> PathBuf {
    home().join(".hark").join("config.toml")
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

    // Stage 2 (26/08): consolidate under ~/.hark.
    consolidate_under_dot_hark(&home());
}

/// Gather every machine-global artifact under `~/.hark`, mirroring the
/// `~/.claude` convention: the sqlite index, state.json, the persona, the
/// whisper models AND config.toml live in one discoverable root instead of
/// an OS-specific data dir plus a separate config dir. Rename-based, cheap
/// no-op on every later boot; paths inside user files follow the move.
pub fn consolidate_under_dot_hark(home: &Path) {
    let hark = home.join(".hark");
    let old_data = if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support").join("hark")
    } else {
        home.join(".local").join("share").join("hark")
    };
    migrate_dir(&old_data, &hark);

    // The config FILE joins the same root (dir-migration cannot help: the
    // target already exists once data moved).
    let old_cfg = home.join(".config").join("hark").join("config.toml");
    let new_cfg = hark.join("config.toml");
    if old_cfg.is_file() && !new_cfg.exists() {
        let _ = std::fs::create_dir_all(&hark);
        if std::fs::rename(&old_cfg, &new_cfg).is_ok() {
            let _ = std::fs::remove_dir(old_cfg.parent().unwrap());
        }
    }

    // Absolute paths written by us into user files follow the move — the
    // statusline bridge in ~/.claude/settings.json above all (a stale
    // script path would silently kill the subscription meter).
    rewrite_legacy_paths(&new_cfg, &old_data, &hark);
    rewrite_legacy_paths(&home.join(".claude").join("settings.json"), &old_data, &hark);
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
        // One discoverable root, mirroring ~/.claude (26/08): index.db,
        // state.json, persona, models and config.toml all live here.
        let dir = home().join(".hark");
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

/// Refuse a config.toml that would not load — TOML that does not parse,
/// or a value of the wrong type — naming the line. `Config::load` falls
/// back to defaults on a broken file, silently, which is the one thing an
/// editor inside the app must never be able to cause.
pub fn validate_toml(text: &str) -> Result<(), String> {
    let line_of = |offset: usize| text[..offset.min(text.len())].matches('\n').count() + 1;
    text.parse::<toml_edit::DocumentMut>().map_err(|e| {
        let at = e.span().map(|s| line_of(s.start)).unwrap_or(1);
        format!("linha {at}: {}", e.message())
    })?;
    toml::from_str::<Config>(text).map(|_| ()).map_err(|e| {
        let at = e.span().map(|s| line_of(s.start)).unwrap_or(1);
        format!("linha {at}: {}", e.message())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_preference_is_written_even_when_the_file_never_had_the_key() {
        // Every config in the wild predates these keys, so persisting a
        // pill depends on the patch CREATING them, not merely updating.
        let out = patch_toml(
            "model = \"sonnet\"\nworker_mode = \"auto\"\n",
            &serde_json::json!({ "worker_model": "claude-opus-5", "worker_effort": "max" }),
        )
        .unwrap();
        let cfg: Config = toml::from_str(&out).unwrap();
        assert_eq!(cfg.worker_model, "claude-opus-5");
        assert_eq!(cfg.worker_effort, "max");
        // and what was already there is left alone
        assert_eq!(cfg.worker_mode, "auto");
    }

    #[test]
    fn the_composer_pills_are_preferences_and_survive_a_restart() {
        // Picking a model and an effort and finding them blank on the next
        // boot was the complaint: these are preferences, not session state.
        let cfg: Config = toml::from_str(
            "worker_mode = \"auto\"\nworker_model = \"claude-opus-5\"\nworker_effort = \"max\"\n",
        )
        .unwrap();
        assert_eq!(cfg.worker_mode, "auto");
        assert_eq!(cfg.worker_model, "claude-opus-5");
        assert_eq!(cfg.worker_effort, "max");
    }

    #[test]
    fn a_config_written_before_those_fields_existed_still_loads() {
        let cfg: Config = toml::from_str("worker_mode = \"plan\"\n").unwrap();
        assert_eq!(cfg.worker_model, "");
        assert_eq!(cfg.worker_effort, "");
    }

    #[test]
    fn an_agents_table_round_trips_and_overrides_one_field() {
        let file = r#"
model = "sonnet"

[agent]
plugin = "claude"
ask = "gemini"

[agents.gemini]
args = ["--experimental-acp"]

[agents.codex]
cmd = "codex-acp"
name = "Codex"
"#;
        let config: Config = toml::from_str(file).expect("parses");
        assert_eq!(config.agent.ask, "gemini");
        let merged = crate::domain::agents::merge(&config.agents);
        let gemini = crate::domain::agents::resolve(&merged, "gemini").expect("gemini");
        assert_eq!(gemini.args, vec!["--experimental-acp"]);
        // Everything the user did not restate survives the merge.
        assert_eq!(gemini.cmd, "gemini");
        let codex = crate::domain::agents::resolve(&merged, "codex").expect("codex");
        assert_eq!(codex.cmd, "codex-acp");
        assert!(codex.enabled, "a declared agent is on unless it says otherwise");
    }

    #[test]
    fn the_catalog_switch_turns_a_built_in_on_and_off_by_dotted_key() {
        // Settings › Plugins writes exactly this patch. claude-acp ships
        // off and its id carries a hyphen, which the dotted path and the
        // merge with the built-in must both survive — otherwise the one
        // card the switch was built for is the one it cannot turn on.
        let on = patch_toml("", &serde_json::json!({ "agents.claude-acp.enabled": true }))
            .expect("patch on");
        let config: Config = toml::from_str(&on).expect("parses");
        let merged = crate::domain::agents::merge(&config.agents);
        let acp = crate::domain::agents::resolve(&merged, "claude-acp").expect("claude-acp");
        assert!(acp.enabled, "the switch turned it on:\n{on}");
        assert_eq!(acp.cmd, "claude-agent-acp", "the built-in's other fields survive");

        let off = patch_toml(&on, &serde_json::json!({ "agents.claude-acp.enabled": false }))
            .expect("patch off");
        let config: Config = toml::from_str(&off).expect("parses");
        let merged = crate::domain::agents::merge(&config.agents);
        let acp = crate::domain::agents::resolve(&merged, "claude-acp").expect("claude-acp");
        assert!(!acp.enabled, "the switch turned it off:\n{off}");
        // The agent nobody touched keeps its own default.
        let claude = crate::domain::agents::resolve(&merged, "claude").expect("claude");
        assert!(claude.enabled);
    }

    #[test]
    fn a_config_with_no_agents_table_still_knows_every_builtin() {
        let config: Config = toml::from_str("model = \"sonnet\"\n").expect("parses");
        assert!(config.agents.is_empty());
        // claude, gemini, claude-acp, codex, deepseek, kiro, antigravity.
        assert_eq!(crate::domain::agents::merge(&config.agents).len(), 7);
    }

    /// A config file written before a field existed must still get that
    /// field's shipped default. Anything else means a new feature arrives
    /// switched off for everyone who already installed — silently.
    #[test]
    fn a_config_written_before_a_field_existed_gets_its_default() {
        let old_file = "model = \"sonnet\"\nlanguage = \"pt\"\n";
        let config: Config = toml::from_str(old_file).expect("parses");
        assert!(config.auto_update, "auto_update must default to on");
        assert_eq!(config.worker_budget_usd, 2.0, "the $2 guardrail survives");
        // FASE 8.6: a project's own ceiling beats the global one — longest
        // prefix wins, and 0 switches the cap off for that project only.
        let mut cfg = Config::default();
        cfg.project_budgets.insert("~/Projects/caro".into(), 8.0);
        cfg.project_budgets.insert("~/Projects/caro/sub".into(), 0.0);
        let at =
            |p: &str| cfg.spawn_limits_for(std::path::Path::new(&expand_home(p))).max_budget_usd;
        assert_eq!(at("~/Projects/caro"), Some(8.0));
        assert_eq!(at("~/Projects/caro/api"), Some(8.0), "prefix covers children");
        assert_eq!(at("~/Projects/caro/sub"), None, "0 = uncapped, longest prefix wins");
        assert_eq!(at("~/Projects/outro"), Some(2.0), "global default elsewhere");
        assert_eq!(config.assistant_name, "Hark");
        assert_eq!(config.hotkey, "cmd+shift+space");
        // What the file DID say still wins.
        assert_eq!(config.model, "sonnet");
    }

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

    /// Stage 2 (26/08): everything global consolidates under ~/.hark,
    /// mirroring ~/.claude — the user looked for the sqlite and the state
    /// where every other CLI keeps theirs and found an OS-specific dir.
    #[test]
    fn consolidation_gathers_data_and_config_under_dot_hark() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path();
        // Old macOS data dir with the real artifacts…
        let old_data = home.join("Library").join("Application Support").join("hark");
        std::fs::create_dir_all(old_data.join("models")).unwrap();
        std::fs::write(old_data.join("index.db"), "sqlite").unwrap();
        std::fs::write(old_data.join("state.json"), "{}").unwrap();
        // …the old config file…
        let old_cfg = home.join(".config").join("hark");
        std::fs::create_dir_all(&old_cfg).unwrap();
        std::fs::write(old_cfg.join("config.toml"), "theme = \"hark\"\n").unwrap();
        // …and a bridge install pointing at the old script path.
        let claude = home.join(".claude");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::write(
            claude.join("settings.json"),
            format!(
                "{{\"statusLine\":{{\"command\":\"{}\"}}}}",
                old_data.join("statusline-bridge.sh").display()
            ),
        )
        .unwrap();

        consolidate_under_dot_hark(home);

        let hark = home.join(".hark");
        assert_eq!(std::fs::read_to_string(hark.join("index.db")).unwrap(), "sqlite");
        assert!(hark.join("models").is_dir());
        assert_eq!(
            std::fs::read_to_string(hark.join("config.toml")).unwrap(),
            "theme = \"hark\"\n"
        );
        assert!(!old_data.exists(), "old data dir is gone");
        let settings = std::fs::read_to_string(claude.join("settings.json")).unwrap();
        assert!(
            settings.contains(&hark.join("statusline-bridge.sh").display().to_string()),
            "bridge path follows the move: {settings}"
        );
        // Second boot: clean no-op.
        consolidate_under_dot_hark(home);
        assert_eq!(std::fs::read_to_string(hark.join("index.db")).unwrap(), "sqlite");
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

/// The config as the windows see it (`config_read`): every value of an
/// agent's `env` masked, the key names kept. Tokens stay in the file and
/// in the processes that need them, never in a webview's memory.
pub fn redacted_json(config: &Config) -> serde_json::Value {
    let mut json = serde_json::to_value(config).unwrap_or_default();
    if let Some(agents) = json.get_mut("agents").and_then(serde_json::Value::as_object_mut) {
        for entry in agents.values_mut() {
            if let Some(env) = entry.get_mut("env").and_then(serde_json::Value::as_object_mut) {
                for value in env.values_mut() {
                    *value = serde_json::Value::String("•••".into());
                }
            }
        }
    }
    json
}

#[cfg(test)]
mod redaction_tests {
    use super::*;

    #[test]
    fn agent_env_values_never_reach_the_windows() {
        // Gateway tokens live in [agents.<id>] env. Every window reads the
        // config; only the key names are its business (the catalog shows
        // env_keys the same way).
        let cfg: Config = toml::from_str(
            "[agents.twin]\nplugin = \"claude\"\nenv = { ANTHROPIC_AUTH_TOKEN = \"sk-live-secret\", ANTHROPIC_BASE_URL = \"https://gw.example\" }\n",
        )
        .unwrap();
        let json = redacted_json(&cfg);
        assert!(!json.to_string().contains("sk-live-secret"), "{json}");
        let env = &json["agents"]["twin"]["env"];
        assert!(env.get("ANTHROPIC_AUTH_TOKEN").is_some(), "key names stay: {env}");
        assert_eq!(json["agents"]["twin"]["plugin"], "claude");
    }
}

#[cfg(test)]
mod raw_edit_tests {
    use super::*;

    #[test]
    fn a_config_that_parses_is_accepted_as_is() {
        assert_eq!(validate_toml("model = \"sonnet\"\n[agents.twin]\nplugin = \"claude\"\n"), Ok(()));
    }

    #[test]
    fn the_empty_file_is_a_valid_config_of_defaults() {
        assert_eq!(validate_toml(""), Ok(()));
    }

    #[test]
    fn a_syntax_slip_is_refused_and_named_by_line() {
        // A broken file loads as ALL DEFAULTS today, silently: the editor
        // must refuse to write one, and say where.
        let err = validate_toml("model = \"sonnet\"\n[agents.twin\nplugin = \"claude\"\n").unwrap_err();
        assert!(err.contains("linha 2"), "{err}");
    }

    #[test]
    fn a_wrong_type_is_refused_with_its_line() {
        let err = validate_toml("model = \"sonnet\"\nworker_budget_usd = \"dois\"\n").unwrap_err();
        assert!(err.contains("linha 2"), "{err}");
    }
}

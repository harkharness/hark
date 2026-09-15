//! The agent registry: which backends Hark knows about, which of them are
//! actually on this machine, and which one drives a new chat.
//!
//! Pure. Detection (`which`) and spawning live in the shell; everything
//! here decides from a list of ids the shell says it found.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One backend Hark can drive. Built-in entries ship with the app; the
/// user's config can add entries or override any field of a built-in.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct AgentEntry {
    /// Registry key: "claude", "gemini", "claude-acp".
    pub id: String,
    /// Human name for the catalog.
    pub name: String,
    /// Which plugin speaks to it: "claude" (native) or "acp".
    pub plugin: String,
    /// Binary to look for and to spawn.
    pub cmd: String,
    pub args: Vec<String>,
    /// A backend the user switched off never appears, detected or not.
    pub enabled: bool,
    /// The file this agent reads as standing instructions ("CLAUDE.md").
    pub memory_file: Option<String>,
    /// What to tell the user when the agent answers "not logged in".
    pub login_hint: Option<String>,
    /// Model ids per tier, for the model pill and the router.
    pub models: BTreeMap<String, String>,
    /// This agent's id in the ACP registry (registry.json), when it has
    /// one — the source of truth for what version is current. Not ours:
    /// codex is `codex-acp` there.
    pub registry: Option<String>,
    /// Base environment for every process of this agent: a gateway URL,
    /// the token it wants. This is how the SAME claude plugin drives a
    /// company LiteLLM (`ANTHROPIC_BASE_URL` + `ANTHROPIC_AUTH_TOKEN`) as
    /// a second entry. Values are the user's own config; never logged.
    pub env: BTreeMap<String, String>,
}

impl Default for AgentEntry {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            plugin: "acp".into(),
            cmd: String::new(),
            args: Vec::new(),
            enabled: true,
            memory_file: None,
            login_hint: None,
            models: BTreeMap::new(),
            registry: None,
            env: BTreeMap::new(),
        }
    }
}

impl AgentEntry {
    /// The base env as the shape `Command::envs` and `SessionSpec` take.
    pub fn env_pairs(&self) -> Vec<(String, String)> {
        self.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}

fn models(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// The backends Hark ships knowing about. Claude is the native plugin;
/// everything else speaks ACP, so each costs a registry line, not a
/// crate. Commands are the ones each project documents for its ACP mode.
pub fn builtins() -> Vec<AgentEntry> {
    vec![
        AgentEntry {
            id: "claude".into(),
            name: "Claude Code".into(),
            plugin: "claude".into(),
            cmd: "claude".into(),
            memory_file: Some("CLAUDE.md".into()),
            login_hint: Some("claude /login".into()),
            models: models(&[
                ("light", "haiku"),
                ("standard", "sonnet"),
                ("heavy", "opus"),
                ("max", "fable"),
            ]),
            ..Default::default()
        },
        AgentEntry {
            id: "gemini".into(),
            name: "Gemini CLI".into(),
            plugin: "acp".into(),
            cmd: "gemini".into(),
            args: vec!["--acp".into()],
            memory_file: Some("GEMINI.md".into()),
            login_hint: Some("gemini".into()),
            registry: Some("gemini".into()),
            models: models(&[
                ("light", "gemini-2.5-flash-lite"),
                ("standard", "gemini-2.5-flash"),
                ("heavy", "gemini-2.5-pro"),
                ("max", "gemini-2.5-pro"),
            ]),
            ..Default::default()
        },
        // The ACP org's adapter over the Claude Agent SDK
        // (agentclientprotocol/claude-agent-acp). Zed's claude-code-acp was
        // its first home; that package is deprecated and never priced a
        // turn, this one reports usage and cumulative cost.
        AgentEntry {
            id: "claude-acp".into(),
            name: "Claude Code (ACP)".into(),
            plugin: "acp".into(),
            cmd: "claude-agent-acp".into(),
            // Off by default: it drives the same subscription as the native
            // plugin, which is richer. It exists to prove the contract.
            enabled: false,
            memory_file: Some("CLAUDE.md".into()),
            login_hint: Some("claude /login".into()),
            registry: Some("claude-acp".into()),
            // Claude's own names: the adapter offers `haiku`, `sonnet`,
            // `opus[1m]`… and the ACP plugin matches them by family. An
            // ACP entry WITHOUT a table gets no model at all (see
            // `model_id`), which would lose this one its cheap lane.
            models: models(&[
                ("light", "haiku"),
                ("standard", "sonnet"),
                ("heavy", "opus"),
                ("max", "fable"),
            ]),
            ..Default::default()
        },
        // OpenAI Codex through Zed's adapter (zed-industries/codex-acp):
        // the user's ChatGPT login, or any OpenAI-compatible base URL the
        // codex config points at — a LiteLLM, DeepSeek's API.
        AgentEntry {
            id: "codex".into(),
            name: "Codex CLI".into(),
            plugin: "acp".into(),
            cmd: "codex-acp".into(),
            memory_file: Some("AGENTS.md".into()),
            login_hint: Some("codex login".into()),
            registry: Some("codex-acp".into()),
            ..Default::default()
        },
        // The DeepSeek harness ships an ACP profile (dsh --profile acp).
        AgentEntry {
            id: "deepseek".into(),
            name: "DeepSeek Harness".into(),
            plugin: "acp".into(),
            cmd: "dsh".into(),
            args: vec!["--profile".into(), "acp".into()],
            login_hint: Some("dsh".into()),
            ..Default::default()
        },
        // AWS Kiro's CLI speaks ACP natively (kiro.dev/docs/cli/acp).
        AgentEntry {
            id: "kiro".into(),
            name: "Kiro CLI".into(),
            plugin: "acp".into(),
            cmd: "kiro-cli".into(),
            args: vec!["acp".into()],
            login_hint: Some("kiro-cli login".into()),
            ..Default::default()
        },
        // Google Antigravity's ACP server, the path Google points
        // individual Gemini CLI accounts at. The registry ships it as a
        // .par archive with no installer: unzip, put it on PATH.
        AgentEntry {
            id: "antigravity".into(),
            name: "Google Antigravity".into(),
            plugin: "acp".into(),
            cmd: "agy_acp_server.par".into(),
            registry: Some("antigravity-acp".into()),
            ..Default::default()
        },
    ]
}

/// Built-ins with the user's table merged over them, field by field for
/// known ids, appended for unknown ones. The user never has to restate a
/// whole entry to change one thing.
pub fn merge(user: &BTreeMap<String, AgentEntry>) -> Vec<AgentEntry> {
    let mut out = builtins();
    for (id, over) in user {
        let over = AgentEntry { id: id.clone(), ..over.clone() };
        match out.iter_mut().find(|e| &e.id == id) {
            Some(base) => {
                if !over.name.is_empty() {
                    base.name = over.name;
                }
                if !over.plugin.is_empty() && over.plugin != "acp" {
                    base.plugin = over.plugin;
                }
                if !over.cmd.is_empty() {
                    base.cmd = over.cmd;
                }
                if !over.args.is_empty() {
                    base.args = over.args;
                }
                base.enabled = over.enabled;
                if over.memory_file.is_some() {
                    base.memory_file = over.memory_file;
                }
                if over.login_hint.is_some() {
                    base.login_hint = over.login_hint;
                }
                if over.registry.is_some() {
                    base.registry = over.registry;
                }
                for (tier, model) in over.models {
                    base.models.insert(tier, model);
                }
                for (key, value) in over.env {
                    base.env.insert(key, value);
                }
            }
            None => out.push(over),
        }
    }
    out
}

/// The entry for an id, whatever the user did to it.
pub fn resolve<'a>(entries: &'a [AgentEntry], id: &str) -> Option<&'a AgentEntry> {
    entries.iter().find(|e| e.id == id)
}

/// Usable = the user left it on AND the binary is on this machine.
pub fn usable<'a>(entries: &'a [AgentEntry], detected: &[String]) -> Vec<&'a AgentEntry> {
    entries
        .iter()
        .filter(|e| e.enabled && detected.iter().any(|d| d == &e.id))
        .collect()
}

/// Which backend a new chat opens with: the user's pick when it is
/// usable, else claude when it is here, else the first usable one.
/// Returns None on a machine with no agent at all.
pub fn default_agent(
    entries: &[AgentEntry],
    detected: &[String],
    preference: &str,
) -> Option<String> {
    let usable = usable(entries, detected);
    let has = |id: &str| usable.iter().any(|e| e.id == id);
    if !preference.is_empty() && has(preference) {
        return Some(preference.to_string());
    }
    if has("claude") {
        return Some("claude".into());
    }
    usable.first().map(|e| e.id.clone())
}

/// Order for the cheap ask/gate lane: the same choice as a new chat,
/// then everything else usable — a fallback list, not a single answer.
pub fn ask_order(entries: &[AgentEntry], detected: &[String], preference: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(first) = default_agent(entries, detected, preference) {
        out.push(first);
    }
    for e in usable(entries, detected) {
        if !out.contains(&e.id) {
            out.push(e.id.clone());
        }
    }
    out
}

/// An agent named out loud: "abre um chat gemini no projeto X". Matches
/// the id or the first word of the name, and only as a WHOLE word — a
/// task called "geminis do time" must not pick a backend.
pub fn spoken_agent(utterance: &str, entries: &[AgentEntry]) -> Option<String> {
    let said = utterance.to_lowercase();
    let words: Vec<&str> = said
        .split(|c: char| !c.is_alphanumeric() && c != '-')
        .filter(|w| !w.is_empty())
        .collect();
    entries
        .iter()
        .find(|e| {
            let first = e.name.split_whitespace().next().unwrap_or("").to_lowercase();
            words.contains(&e.id.as_str()) || (!first.is_empty() && words.contains(&first.as_str()))
        })
        .map(|e| e.id.clone())
}

/// The four tiers every pill and router speak in.
pub const TIERS: [&str; 4] = ["light", "standard", "heavy", "max"];

/// Which tier a requested model stands for: the tier's own key, or the
/// GLOBAL table's name for it ("haiku" is claude's light). Anything else
/// is an explicit model id and belongs to no tier.
pub fn tier_of(requested: &str, tiers: &crate::domain::intent::Models) -> Option<&'static str> {
    if requested.is_empty() {
        return None;
    }
    if let Some(key) = TIERS.iter().find(|t| **t == requested) {
        return Some(key);
    }
    tier_names(tiers)
        .into_iter()
        .find(|(_, name)| *name == requested)
        .map(|(key, _)| key)
}

fn tier_names(tiers: &crate::domain::intent::Models) -> [(&'static str, &str); 4] {
    [("light", &tiers.light), ("standard", &tiers.standard), ("heavy", &tiers.heavy), ("max", &tiers.max)]
}

/// The model id to ask THIS agent for.
///
/// One pill drives every agent, so what it says is a tier — its key, or
/// the global table's name for it — and each registry line says what that
/// tier is called on its own agent. An explicit id passes through
/// untouched. An agent with a table but not this tier, or an ACP agent
/// with no table at all, gets NO model and runs its own default: a claude
/// name sent to codex is refused or ignored, and either way the pill would
/// be lying. The global table is claude's vocabulary, so a claude entry
/// without a table of its own (a gateway twin) speaks it as is.
pub fn model_id(
    entry: &AgentEntry,
    requested: &str,
    tiers: &crate::domain::intent::Models,
) -> Option<String> {
    if requested.is_empty() {
        return None;
    }
    let Some(tier) = tier_of(requested, tiers) else {
        return Some(requested.to_string());
    };
    if !entry.models.is_empty() {
        return entry.models.get(tier).cloned();
    }
    (entry.plugin == "claude").then(|| {
        tier_names(tiers)
            .into_iter()
            .find(|(key, _)| *key == tier)
            .map(|(_, name)| name.to_string())
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(entries: &[(&str, AgentEntry)]) -> BTreeMap<String, AgentEntry> {
        entries.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn ships_knowing_claude_and_every_acp_agent_on_the_market() {
        let ids: Vec<String> = builtins().into_iter().map(|e| e.id).collect();
        assert_eq!(ids, vec!["claude", "gemini", "claude-acp", "codex", "deepseek", "kiro", "antigravity"]);
    }

    /// The commands are the ones each project documents for ACP mode
    /// (gemini docs/cli/acp-mode, zed-industries/codex-acp, dsh-acp,
    /// kiro.dev/docs/cli/acp) — a registry line, not a crate, per agent.
    #[test]
    fn builtins_speak_acp_with_their_documented_commands() {
        let all = builtins();
        let cmdline = |id: &str| {
            let e = resolve(&all, id).unwrap();
            assert_eq!(e.plugin, "acp", "{id}");
            std::iter::once(e.cmd.clone()).chain(e.args.iter().cloned()).collect::<Vec<_>>().join(" ")
        };
        assert_eq!(cmdline("gemini"), "gemini --acp");
        assert_eq!(cmdline("codex"), "codex-acp");
        assert_eq!(cmdline("deepseek"), "dsh --profile acp");
        assert_eq!(cmdline("kiro"), "kiro-cli acp");
        // The ACP org took the Claude adapter over from Zed and renamed the
        // binary; the old one is deprecated and never reported cost.
        assert_eq!(cmdline("claude-acp"), "claude-agent-acp");
        // Google's successor to individual-account Gemini, as the ACP
        // registry distributes it (a .par archive, run from PATH).
        assert_eq!(cmdline("antigravity"), "agy_acp_server.par");
    }

    #[test]
    fn env_merges_key_by_key_like_models() {
        let mut env = BTreeMap::new();
        env.insert("ANTHROPIC_BASE_URL".to_string(), "http://litellm:4000".to_string());
        let table = user(&[("claude", AgentEntry { env, ..Default::default() })]);
        let merged = merge(&table);
        let claude = resolve(&merged, "claude").unwrap();
        assert_eq!(claude.env.get("ANTHROPIC_BASE_URL").map(String::as_str), Some("http://litellm:4000"));
        assert_eq!(claude.env_pairs(), vec![("ANTHROPIC_BASE_URL".to_string(), "http://litellm:4000".to_string())]);
        // The rest of the builtin is untouched by an env-only override.
        assert_eq!(claude.plugin, "claude");
        assert_eq!(claude.cmd, "claude");
    }

    /// The LiteLLM recipe: a second entry that reuses the native claude
    /// plugin with a gateway in its env, alongside the plain "claude".
    #[test]
    fn a_gateway_entry_reuses_the_claude_plugin_with_its_own_env() {
        let mut env = BTreeMap::new();
        env.insert("ANTHROPIC_BASE_URL".to_string(), "http://litellm:4000".to_string());
        env.insert("ANTHROPIC_AUTH_TOKEN".to_string(), "sk-static".to_string());
        let table = user(&[(
            "claude-litellm",
            AgentEntry {
                plugin: "claude".into(),
                cmd: "claude".into(),
                name: "Claude via LiteLLM".into(),
                env,
                ..Default::default()
            },
        )]);
        let merged = merge(&table);
        let gw = resolve(&merged, "claude-litellm").unwrap();
        assert_eq!(gw.plugin, "claude", "a user entry keeps the plugin it declares");
        assert_eq!(gw.env.len(), 2);
        assert!(resolve(&merged, "claude").unwrap().env.is_empty(), "the plain entry is not touched");
    }

    #[test]
    fn the_native_claude_plugin_is_not_acp() {
        let all = builtins();
        assert_eq!(resolve(&all, "claude").unwrap().plugin, "claude");
        assert_eq!(resolve(&all, "gemini").unwrap().plugin, "acp");
    }

    #[test]
    fn claude_over_acp_ships_off_because_the_native_plugin_is_richer() {
        assert!(!resolve(&builtins(), "claude-acp").unwrap().enabled);
        assert!(resolve(&builtins(), "gemini").unwrap().enabled);
    }

    #[test]
    fn overriding_one_field_keeps_the_rest_of_the_builtin() {
        let table = user(&[(
            "gemini",
            AgentEntry { args: vec!["--experimental-acp".into()], ..Default::default() },
        )]);
        let merged = merge(&table);
        let gemini = resolve(&merged, "gemini").unwrap();
        assert_eq!(gemini.args, vec!["--experimental-acp"]);
        // untouched:
        assert_eq!(gemini.cmd, "gemini");
        assert_eq!(gemini.memory_file.as_deref(), Some("GEMINI.md"));
        assert_eq!(gemini.models.get("heavy").map(String::as_str), Some("gemini-2.5-pro"));
    }

    #[test]
    fn switching_a_builtin_off_is_an_override_like_any_other() {
        let table = user(&[("gemini", AgentEntry { enabled: false, ..Default::default() })]);
        assert!(!resolve(&merge(&table), "gemini").unwrap().enabled);
    }

    #[test]
    fn an_unknown_id_is_a_new_agent_not_an_error() {
        let table = user(&[(
            "codex",
            AgentEntry { cmd: "codex-acp".into(), name: "Codex".into(), ..Default::default() },
        )]);
        let merged = merge(&table);
        let codex = resolve(&merged, "codex").unwrap();
        assert_eq!(codex.plugin, "acp");
        assert_eq!(codex.cmd, "codex-acp");
    }

    #[test]
    fn a_backend_is_usable_only_when_enabled_and_present() {
        let all = builtins();
        let detected = vec!["gemini".to_string(), "claude-acp".to_string()];
        let ids: Vec<&str> = usable(&all, &detected).iter().map(|e| e.id.as_str()).collect();
        // claude-acp is detected but ships disabled; claude is enabled but absent.
        assert_eq!(ids, vec!["gemini"]);
    }

    #[test]
    fn claude_drives_a_new_chat_when_it_is_here() {
        let all = builtins();
        let detected = vec!["claude".to_string(), "gemini".to_string()];
        assert_eq!(default_agent(&all, &detected, "").as_deref(), Some("claude"));
    }

    #[test]
    fn without_claude_the_first_usable_backend_takes_over() {
        let all = builtins();
        let detected = vec!["gemini".to_string()];
        assert_eq!(default_agent(&all, &detected, "").as_deref(), Some("gemini"));
    }

    #[test]
    fn a_preference_wins_but_only_while_it_is_usable() {
        let all = builtins();
        let both = vec!["claude".to_string(), "gemini".to_string()];
        assert_eq!(default_agent(&all, &both, "gemini").as_deref(), Some("gemini"));
        // The user picked gemini and then uninstalled it: fall back, don't break.
        let only_claude = vec!["claude".to_string()];
        assert_eq!(default_agent(&all, &only_claude, "gemini").as_deref(), Some("claude"));
    }

    #[test]
    fn a_machine_with_no_agent_has_no_default() {
        assert_eq!(default_agent(&builtins(), &[], ""), None);
    }

    #[test]
    fn the_ask_lane_falls_back_through_every_usable_backend() {
        let all = builtins();
        let detected = vec!["claude".to_string(), "gemini".to_string()];
        assert_eq!(ask_order(&all, &detected, "gemini"), vec!["gemini", "claude"]);
        assert_eq!(ask_order(&all, &[], ""), Vec::<String>::new());
    }

    #[test]
    fn naming_an_agent_out_loud_picks_it() {
        let all = builtins();
        assert_eq!(
            spoken_agent("abre um chat gemini no projeto hark", &all).as_deref(),
            Some("gemini")
        );
        assert_eq!(spoken_agent("abre um chat com o claude", &all).as_deref(), Some("claude"));
    }

    #[test]
    fn a_word_that_merely_contains_a_name_is_not_a_pick() {
        // "geminis" is not "gemini"; whole words only.
        assert_eq!(spoken_agent("abre o chat dos geminis do time", &builtins()), None);
        assert_eq!(spoken_agent("continua a migração", &builtins()), None);
    }
}

#[cfg(test)]
mod model_ids {
    //! The tier table is how one pill drives every agent: "light" (or the
    //! global table's name for it, "haiku") becomes THIS agent's own id.
    use super::*;
    use crate::domain::intent::Models;

    fn tiers() -> Models {
        Models::default()
    }

    fn builtin(id: &str) -> AgentEntry {
        resolve(&builtins(), id).unwrap().clone()
    }

    #[test]
    fn a_tier_key_resolves_to_the_agents_own_model() {
        assert_eq!(model_id(&builtin("gemini"), "light", &tiers()).as_deref(), Some("gemini-2.5-flash-lite"));
        assert_eq!(model_id(&builtin("gemini"), "max", &tiers()).as_deref(), Some("gemini-2.5-pro"));
        assert_eq!(model_id(&builtin("claude"), "heavy", &tiers()).as_deref(), Some("opus"));
    }

    #[test]
    fn a_global_tier_name_resolves_through_its_tier() {
        // The window default says "haiku" — claude's light. In a gemini
        // chat that is flash-lite, never a claude name sent to gemini.
        assert_eq!(model_id(&builtin("gemini"), "haiku", &tiers()).as_deref(), Some("gemini-2.5-flash-lite"));
        assert_eq!(model_id(&builtin("gemini"), "fable", &tiers()).as_deref(), Some("gemini-2.5-pro"));
        assert_eq!(model_id(&builtin("claude"), "haiku", &tiers()).as_deref(), Some("haiku"));
    }

    #[test]
    fn an_explicit_model_id_passes_through_untouched() {
        assert_eq!(model_id(&builtin("gemini"), "gemini-2.5-pro", &tiers()).as_deref(), Some("gemini-2.5-pro"));
        assert_eq!(model_id(&builtin("codex"), "gpt-5-codex", &tiers()).as_deref(), Some("gpt-5-codex"));
        assert_eq!(model_id(&builtin("claude"), "claude-fable-5-1", &tiers()).as_deref(), Some("claude-fable-5-1"));
    }

    #[test]
    fn an_acp_agent_without_a_table_gets_no_model_rather_than_a_claude_name() {
        // Codex has no table yet: "haiku" must not reach it (it would be
        // refused, or silently ignored, either way a lie on the pill).
        // No model = the agent's own default.
        assert_eq!(model_id(&builtin("codex"), "haiku", &tiers()), None);
        assert_eq!(model_id(&builtin("codex"), "light", &tiers()), None);
    }

    #[test]
    fn a_claude_twin_without_a_table_speaks_the_global_names() {
        // The global table IS claude's vocabulary: a second claude entry
        // (a LiteLLM twin) with no table of its own takes it as is.
        let twin = AgentEntry { id: "claude-litellm".into(), plugin: "claude".into(), cmd: "claude".into(), ..Default::default() };
        assert_eq!(model_id(&twin, "light", &tiers()).as_deref(), Some("haiku"));
        assert_eq!(model_id(&twin, "opus", &tiers()).as_deref(), Some("opus"));
    }

    #[test]
    fn a_table_missing_the_tier_yields_no_model() {
        let partial = AgentEntry { id: "x".into(), models: models(&[("light", "mini")]), ..Default::default() };
        assert_eq!(model_id(&partial, "light", &tiers()).as_deref(), Some("mini"));
        assert_eq!(model_id(&partial, "sonnet", &tiers()), None);
    }

    #[test]
    fn nothing_requested_is_nothing_resolved() {
        // An empty pick means "the agent's default", whatever the tables say.
        assert_eq!(model_id(&builtin("gemini"), "", &tiers()), None);
        assert_eq!(tier_of("", &tiers()), None);
    }

    #[test]
    fn the_claude_adapter_over_acp_carries_claudes_own_table() {
        // claude-agent-acp offers `haiku`, `sonnet`, `opus[1m]`… — claude's
        // names, matched by family in the ACP plugin. Without a table the
        // ACP rule above would send it NO model and lose the cheap lane.
        let acp = builtin("claude-acp");
        assert_eq!(acp.models.get("light").map(String::as_str), Some("haiku"));
        assert_eq!(acp.models.len(), 4);
        assert_eq!(model_id(&acp, "haiku", &tiers()).as_deref(), Some("haiku"));
    }

    #[test]
    fn a_tier_is_known_by_its_key_or_by_the_global_tables_name() {
        assert_eq!(tier_of("light", &tiers()), Some("light"));
        assert_eq!(tier_of("sonnet", &tiers()), Some("standard"));
        assert_eq!(tier_of("gemini-2.5-pro", &tiers()), None);
    }
}

#[cfg(test)]
mod registry_ids {
    use super::*;

    #[test]
    fn builtins_name_their_entry_in_the_acp_registry() {
        // registry.json is the source of truth for what is current; the
        // ids there are not ours (codex is `codex-acp`).
        let all = builtins();
        let reg = |id: &str| resolve(&all, id).unwrap().registry.clone();
        assert_eq!(reg("claude-acp").as_deref(), Some("claude-acp"));
        assert_eq!(reg("codex").as_deref(), Some("codex-acp"));
        assert_eq!(reg("gemini").as_deref(), Some("gemini"));
        assert_eq!(reg("antigravity").as_deref(), Some("antigravity-acp"));
        // Not in the registry (checked 14/09/2026): nothing to compare against.
        assert_eq!(reg("deepseek"), None);
        assert_eq!(reg("kiro"), None);
        assert_eq!(reg("claude"), None);
    }

    #[test]
    fn a_user_entry_can_name_its_registry_id() {
        let mut table = BTreeMap::new();
        table.insert("goose".to_string(), AgentEntry { cmd: "goose".into(), registry: Some("goose".into()), ..Default::default() });
        let merged = merge(&table);
        assert_eq!(resolve(&merged, "goose").unwrap().registry.as_deref(), Some("goose"));
        // …and override a built-in's.
        let mut over = BTreeMap::new();
        over.insert("codex".to_string(), AgentEntry { registry: Some("codex-next".into()), ..Default::default() });
        assert_eq!(resolve(&merge(&over), "codex").unwrap().registry.as_deref(), Some("codex-next"));
    }
}

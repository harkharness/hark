//! Community token-saving tools: detect what is installed and translate
//! the user's `[assist]` config into per-WORKER env vars — never touching
//! global settings. The hark principle: measure before promoting defaults
//! (each spawn's fingerprint lands in the spend ledger's `outcome`).

use serde::Serialize;

/// What this machine has (plugins + binaries).
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct EcoStatus {
    pub rtk: bool,
    pub ponytail: bool,
    pub caveman: bool,
    pub tokensave: bool,
}

/// Enabled Claude Code plugins out of `~/.claude/settings.json`. Pure:
/// takes the file's text, returns lowercase plugin names.
pub fn parse_enabled_plugins(settings_json: &str) -> Vec<String> {
    let v: serde_json::Value = match serde_json::from_str(settings_json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    match v.get("enabledPlugins") {
        // {"repo/plugin": true} maps and ["plugin"] lists both occur.
        Some(serde_json::Value::Object(map)) => map
            .iter()
            .filter(|(_, on)| on.as_bool() != Some(false))
            .map(|(k, _)| k.to_lowercase())
            .collect(),
        Some(serde_json::Value::Array(list)) => list
            .iter()
            .filter_map(|x| x.as_str())
            .map(str::to_lowercase)
            .collect(),
        _ => Vec::new(),
    }
}

/// Probe the machine: plugins from the settings text, binaries from PATH.
pub fn detect(settings_json: &str) -> EcoStatus {
    let plugins = parse_enabled_plugins(settings_json);
    let has_plugin = |name: &str| plugins.iter().any(|p| p.contains(name));
    let has_bin = |name: &str| {
        std::process::Command::new("which")
            .arg(name)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };
    EcoStatus {
        rtk: has_bin("rtk"),
        ponytail: has_plugin("ponytail"),
        caveman: has_plugin("caveman"),
        tokensave: has_bin("tokensave"),
    }
}

/// Per-worker env vars from what is installed + the user's `[assist]`
/// wishes. Defaults follow the measured evidence: ponytail ON when
/// installed (−20% cost in the reproducible agentic benchmark), caveman
/// OFF for workers (+7% in coding), tokensave OFF (parallel-worker risk).
pub fn eco_envs(
    status: EcoStatus,
    ponytail: Option<&str>,
    caveman: Option<&str>,
    tokensave: Option<&str>,
) -> Vec<(String, String)> {
    let mut envs = Vec::new();
    if status.ponytail {
        let mode = ponytail.unwrap_or("full");
        if mode != "off" {
            envs.push(("PONYTAIL_DEFAULT_MODE".into(), mode.to_string()));
        }
    }
    if status.caveman {
        envs.push((
            "CAVEMAN_DEFAULT_MODE".into(),
            caveman.unwrap_or("off").to_string(),
        ));
    }
    if status.tokensave && tokensave.unwrap_or("off") == "off" {
        envs.push(("TOKENSAVE_DISABLE_SERVER".into(), "true".into()));
    }
    envs
}

/// The ledger fingerprint: which eco tools this spawn ran with — the
/// costs panel compares avg turn cost per fingerprint on REAL load.
pub fn fingerprint(status: EcoStatus, envs: &[(String, String)]) -> String {
    let val = |key: &str, fallback: &str| {
        envs.iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| fallback.to_string())
    };
    format!(
        "eco:rtk={},ponytail={},caveman={}",
        if status.rtk { "on" } else { "off" },
        if status.ponytail { val("PONYTAIL_DEFAULT_MODE", "off") } else { "off".into() },
        if status.caveman { val("CAVEMAN_DEFAULT_MODE", "off") } else { "off".into() },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plugin_maps_and_lists() {
        let map = r#"{"enabledPlugins":{"acme/Ponytail":true,"x/caveman":true,"y/dead":false}}"#;
        let got = parse_enabled_plugins(map);
        assert!(got.iter().any(|p| p.contains("ponytail")));
        assert!(got.iter().any(|p| p.contains("caveman")));
        assert!(!got.iter().any(|p| p.contains("dead")), "disabled stays out");

        let list = r#"{"enabledPlugins":["ponytail"]}"#;
        assert_eq!(parse_enabled_plugins(list), vec!["ponytail"]);
        assert!(parse_enabled_plugins("not json").is_empty());
        assert!(parse_enabled_plugins("{}").is_empty());
    }

    #[test]
    fn envs_follow_evidence_defaults() {
        let all = EcoStatus { rtk: true, ponytail: true, caveman: true, tokensave: true };
        let envs = eco_envs(all, None, None, None);
        assert!(envs.contains(&("PONYTAIL_DEFAULT_MODE".into(), "full".into())));
        assert!(envs.contains(&("CAVEMAN_DEFAULT_MODE".into(), "off".into())));
        assert!(envs.contains(&("TOKENSAVE_DISABLE_SERVER".into(), "true".into())));
    }

    #[test]
    fn user_wishes_override_but_absent_tools_get_nothing() {
        let none = EcoStatus::default();
        assert!(eco_envs(none, Some("full"), None, None).is_empty(), "not installed = no envs");

        let pony = EcoStatus { ponytail: true, ..Default::default() };
        assert!(eco_envs(pony, Some("off"), None, None).is_empty(), "user off wins");
        assert_eq!(
            eco_envs(pony, Some("lazy"), None, None),
            vec![("PONYTAIL_DEFAULT_MODE".to_string(), "lazy".to_string())]
        );
    }

    #[test]
    fn fingerprint_names_the_active_set() {
        let status = EcoStatus { rtk: true, ponytail: true, caveman: false, tokensave: false };
        let envs = eco_envs(status, None, None, None);
        assert_eq!(fingerprint(status, &envs), "eco:rtk=on,ponytail=full,caveman=off");
    }
}

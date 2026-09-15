//! The ACP agent registry (agentclientprotocol/registry, published as
//! `registry.json`): what is CURRENT for each agent, against what this
//! machine has. Pure — the shell fetches the file and runs `--version`.
//!
//! Why it exists: "detected" is not "current". On 14/09/2026 this machine
//! had gemini 0.46.0 with 0.59.0 published, and 0.46's session/new answered
//! a deprecation notice that only an update fixes. Nobody said "update".

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// One agent as the registry publishes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Published {
    /// The registry's own id (`codex-acp`, not Hark's `codex`).
    pub id: String,
    pub version: String,
    /// npm package name, version stripped, when distributed through npx.
    pub package: Option<String>,
    /// The archive URL for THIS platform when distributed as a binary.
    pub archive: Option<String>,
}

/// Read `registry.json`. Anything unparsable is an empty registry, never
/// an error: a check that fails must not take the catalog down with it.
pub fn parse(json: &str, platform: &str) -> Vec<Published> {
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    doc.get("agents")
        .and_then(|a| a.as_array())
        .map(|agents| {
            agents
                .iter()
                .filter_map(|a| {
                    let id = a.get("id")?.as_str()?.to_string();
                    let version = a.get("version")?.as_str()?.to_string();
                    let dist = a.get("distribution");
                    let package = dist
                        .and_then(|d| d.pointer("/npx/package"))
                        .and_then(|p| p.as_str())
                        .map(|p| strip_version(p).to_string());
                    let archive = dist
                        .and_then(|d| d.pointer(&format!("/binary/{platform}/archive")))
                        .and_then(|u| u.as_str())
                        .map(str::to_string);
                    Some(Published { id, version, package, archive })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// "@google/gemini-cli@0.59.0" → "@google/gemini-cli": the scope's own
/// leading `@` is not a version separator.
fn strip_version(spec: &str) -> &str {
    match spec.rfind('@') {
        Some(i) if i > 0 => &spec[..i],
        _ => spec,
    }
}

/// The version in a binary's `--version` output: the first token that
/// reads as one ("@agentclientprotocol/codex-acp 1.11.0" → "1.11.0").
pub fn installed_version(output: &str) -> Option<String> {
    output
        .split(|c: char| c.is_whitespace() || c == '(' || c == ')')
        .map(|t| t.trim_start_matches('v'))
        .find(|t| looks_like_version(t))
        .map(str::to_string)
}

fn looks_like_version(token: &str) -> bool {
    let core = token.split('-').next().unwrap_or_default();
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() >= 2 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Installed against published.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Freshness {
    /// One of the two numbers is missing: nothing to say.
    Unknown,
    /// At or past the published version.
    Current,
    Behind { installed: String, current: String },
}

pub fn freshness(installed: Option<&str>, current: Option<&str>) -> Freshness {
    let (Some(installed), Some(current)) = (installed, current) else {
        return Freshness::Unknown;
    };
    let behind = match numbers(installed).cmp(&numbers(current)) {
        Ordering::Less => true,
        // Same numbers, but a pre-release of them is still behind.
        Ordering::Equal => installed.contains('-') && !current.contains('-'),
        Ordering::Greater => false,
    };
    if behind {
        Freshness::Behind { installed: installed.to_string(), current: current.to_string() }
    } else {
        Freshness::Current
    }
}

fn numbers(version: &str) -> Vec<u64> {
    version
        .split('-')
        .next()
        .unwrap_or_default()
        .split('.')
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

/// The one line that brings a package to the published version. Archives
/// have no such line: the catalog's install text stands.
pub fn update_command(published: &Published) -> Option<String> {
    published.package.as_ref().map(|pkg| format!("npm install -g {pkg}@{}", published.version))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real `registry.json` (v1/latest, 14/09/2026), trimmed to the
    /// agents Hark ships and one binary-distributed example.
    const REGISTRY: &str = include_str!("../../fixtures/acp-registry.trimmed.json");

    fn published(id: &str) -> Published {
        parse(REGISTRY, "darwin-aarch64").into_iter().find(|p| p.id == id).expect(id)
    }

    #[test]
    fn parses_the_real_registry_shape() {
        let gemini = published("gemini");
        assert_eq!(gemini.version, "0.59.0");
        assert_eq!(gemini.package.as_deref(), Some("@google/gemini-cli"));
        let claude = published("claude-acp");
        assert_eq!(claude.version, "0.77.0");
        assert_eq!(claude.package.as_deref(), Some("@agentclientprotocol/claude-agent-acp"));
        // Distributed as an archive per platform, no package at all.
        let agy = published("antigravity-acp");
        assert_eq!(agy.package, None);
        assert!(agy.archive.as_deref().is_some_and(|u| u.contains("darwin-arm64")), "{:?}", agy.archive);
        assert_eq!(parse("not json", "darwin-aarch64"), Vec::<Published>::new());
    }

    #[test]
    fn the_installed_version_is_the_first_version_token_of_the_binarys_own_words() {
        // OBSERVED `--version` outputs on this machine.
        assert_eq!(installed_version("0.46.0\n").as_deref(), Some("0.46.0"));
        assert_eq!(installed_version("@agentclientprotocol/codex-acp 1.11.0").as_deref(), Some("1.11.0"));
        assert_eq!(installed_version("gemini-cli 0.59.0 (abc123)").as_deref(), Some("0.59.0"));
        assert_eq!(installed_version("v1.2.3").as_deref(), Some("1.2.3"));
        assert_eq!(installed_version(""), None);
        assert_eq!(installed_version("command not found"), None);
    }

    #[test]
    fn freshness_compares_numbers_not_letters() {
        assert_eq!(
            freshness(Some("0.46.0"), Some("0.59.0")),
            Freshness::Behind { installed: "0.46.0".into(), current: "0.59.0".into() }
        );
        // 1.9 is BEHIND 1.11 — a string compare would say otherwise.
        assert!(matches!(freshness(Some("1.9.0"), Some("1.11.0")), Freshness::Behind { .. }));
        assert_eq!(freshness(Some("1.11.0"), Some("1.11.0")), Freshness::Current);
        // Ahead of the registry (a pre-release build) is not behind.
        assert_eq!(freshness(Some("0.77.1"), Some("0.77.0")), Freshness::Current);
        // A pre-release of the current version is still behind it.
        assert!(matches!(freshness(Some("0.1.5-rc"), Some("0.1.5")), Freshness::Behind { .. }));
        assert_eq!(freshness(None, Some("1.0.0")), Freshness::Unknown);
        assert_eq!(freshness(Some("1.0.0"), None), Freshness::Unknown);
    }

    #[test]
    fn the_update_line_is_npm_for_packages_and_nothing_for_archives() {
        assert_eq!(
            update_command(&published("gemini")).as_deref(),
            Some("npm install -g @google/gemini-cli@0.59.0")
        );
        assert_eq!(update_command(&published("antigravity-acp")), None);
    }
}

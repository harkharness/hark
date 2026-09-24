//! Reads what `domain::trust` judges for a worker's folder: its Claude
//! Code settings, its `.mcp.json`, and Claude Code's own trust record.

use hark_core::domain::trust::{sources, Sources};
use std::path::{Path, PathBuf};

/// Claude Code's global config, where its trust dialog records answers.
fn record_path() -> PathBuf {
    match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(dir) => PathBuf::from(dir).join(".claude.json"),
        None => PathBuf::from(hark_core::config::expand_home("~/.claude.json")),
    }
}

pub fn project_sources(cwd: &Path) -> Sources {
    let read = |p: PathBuf| std::fs::read_to_string(p).ok();
    let settings: Vec<String> = [".claude/settings.json", ".claude/settings.local.json"]
        .iter()
        .filter_map(|f| read(cwd.join(f)))
        .collect();
    let settings: Vec<&str> = settings.iter().map(String::as_str).collect();
    sources(
        &settings,
        read(cwd.join(".mcp.json")).as_deref(),
        read(record_path()).as_deref(),
        &cwd.to_string_lossy(),
    )
}

/// Said in the chat when a worker opens without the folder's settings.
pub fn untrusted_note(cwd: &Path) -> String {
    format!(
        "configurações do projeto não carregadas: {} traz hooks, permissões, env ou MCP que ainda não foram confiados. Abra `claude` uma vez nessa pasta e aceite a confiança; o próximo chat carrega tudo.",
        cwd.display()
    )
}

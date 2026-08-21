//! Machine-wide state (JSON in the data dir): the active context now, the
//! worker registry later. This is the piece that links all workspaces.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct GlobalState {
    /// Context chosen via `vox use`; None means the config default applies.
    pub active_context: Option<String>,
    /// Dispatched workers across all workspaces (the machine-wide registry).
    pub workers: Vec<crate::domain::memory::WorkerRecord>,
    /// User-editable project directories (the sidebar groups).
    pub projects: Vec<crate::domain::project::Project>,
    /// The mother's persistent work chat session (resumed across restarts).
    pub vox_chat_session: Option<String>,
}

fn state_path(data_dir: &Path) -> PathBuf {
    data_dir.join("state.json")
}

pub fn load(data_dir: &Path) -> GlobalState {
    std::fs::read_to_string(state_path(data_dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(data_dir: &Path, state: &GlobalState) -> anyhow::Result<()> {
    let text = serde_json::to_string_pretty(state)?;
    std::fs::write(state_path(data_dir), text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_and_defaults_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()), GlobalState::default());

        let state = GlobalState {
            active_context: Some("alpha".into()),
            projects: vec![crate::domain::project::Project {
                name: "vox".into(),
                path: "/p/vox".into(),
            }],
            vox_chat_session: Some("s-chat".into()),
            ..GlobalState::default()
        };
        save(dir.path(), &state).unwrap();
        assert_eq!(load(dir.path()), state);

        // Old state files (no projects/vox_chat keys) must still load.
        std::fs::write(
            dir.path().join("state.json"),
            r#"{"active_context":"nu","workers":[]}"#,
        )
        .unwrap();
        let old = load(dir.path());
        assert_eq!(old.active_context.as_deref(), Some("nu"));
        assert_eq!(old.vox_chat_session, None);
    }
}

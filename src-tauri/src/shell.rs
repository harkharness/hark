//! `!command` in the composer: one command, run in the chat's folder,
//! its output handed back to the chat. Not a terminal: nobody types into
//! it, and it ends by itself (`adapters::shell_run`).
//!
//! Only the composer calls this, with what a person typed after `!`.
//! Nothing an agent prints and nothing the microphone hears reaches it
//! (`src/lib/bang.ts`, SECURITY.md).

use hark_core::adapters::shell_run;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri::State;

/// How long a command may run. The terminal pane is the place for
/// anything that stays up: a dev server, a watcher.
const BUDGET: Duration = Duration::from_secs(120);

/// How much of what a command prints is kept, start and end. The same
/// text rides in the next message to the agent, so it is sized for a
/// context window, not for a screen.
const OUTPUT_MAX: usize = 30_000;

/// Commands still running, by the id the chat gave them: what "stop" ends.
#[derive(Default)]
pub struct ShellRuns(Mutex<HashMap<String, u32>>);

#[derive(Serialize)]
pub struct ShellOut {
    output: String,
    /// Bytes left out of the middle of the output.
    omitted: usize,
    /// None when a signal ended it: a stop, or the budget.
    code: Option<i32>,
    timed_out: bool,
    /// The folder it ran in, which is home when the one asked is gone.
    cwd: String,
}

/// Run what was typed after `!`, in `cwd`, and wait for it.
#[tauri::command(async)]
pub fn shell_run(
    runs: State<'_, ShellRuns>,
    id: String,
    command: String,
    cwd: Option<String>,
) -> Result<ShellOut, String> {
    let home = dirs::home_dir().unwrap_or_default();
    let dir = crate::platform::shell_dir(cwd.as_deref(), &home);
    let running = shell_run::start(&crate::platform::default_shell(), &command, &dir)
        .map_err(|e| e.to_string())?;
    runs.0.lock().unwrap().insert(id.clone(), running.pid());
    let ended = running.wait(BUDGET, OUTPUT_MAX);
    runs.0.lock().unwrap().remove(&id);
    Ok(ShellOut {
        output: ended.output,
        omitted: ended.omitted,
        code: ended.code,
        timed_out: ended.timed_out,
        cwd: dir.display().to_string(),
    })
}

/// End a command before it ends by itself.
#[tauri::command]
pub fn shell_stop(runs: State<'_, ShellRuns>, id: String) {
    if let Some(pid) = runs.0.lock().unwrap().get(&id) {
        shell_run::stop(*pid);
    }
}

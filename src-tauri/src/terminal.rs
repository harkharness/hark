//! Real terminals inside the app: one PTY per tab, running the user's own
//! shell (interactive, login), streaming raw bytes to xterm.js on the
//! frontend. The worker feed pane is NOT this — these are actual shells.

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

struct PtyHandle {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

/// Live PTYs by terminal id. Ids are frontend-chosen (per tab).
#[derive(Default)]
pub struct Terminals(Arc<Mutex<HashMap<String, PtyHandle>>>);

#[derive(Serialize, Clone)]
struct TermOut<'a> {
    id: &'a str,
    /// UTF-8 lossy chunk of PTY output (ANSI escapes included).
    data: Option<String>,
    /// True when the shell exited: the frontend closes the tab.
    exit: bool,
}

/// Spawn the user's shell in a fresh PTY. Idempotent: an id that is
/// already running is left untouched (the reopened pane just reattaches).
#[tauri::command]
pub fn term_open(
    app: AppHandle,
    terms: State<'_, Terminals>,
    id: String,
    cwd: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<(), String> {
    let mut map = terms.0.lock().unwrap();
    if map.contains_key(&id) {
        return Ok(());
    }
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: rows.unwrap_or(24),
            cols: cols.unwrap_or(80),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let mut cmd = CommandBuilder::new(&shell);
    // Login shell: the user's real prompt, PATH and aliases, like iTerm.
    cmd.arg("-l");
    if let Some(dir) = cwd.filter(|d| std::path::Path::new(d).is_dir()) {
        cmd.cwd(dir);
    }
    let child = pty.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    let mut reader = pty.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pty.master.take_writer().map_err(|e| e.to_string())?;
    map.insert(
        id.clone(),
        PtyHandle {
            master: pty.master,
            writer,
            child,
        },
    );
    drop(map);

    // Pump PTY output to the UI until the shell dies.
    let registry = terms.0.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let _ = app.emit(
                        "term-out",
                        TermOut {
                            id: &id,
                            data: Some(String::from_utf8_lossy(&buf[..n]).into_owned()),
                            exit: false,
                        },
                    );
                }
            }
        }
        registry.lock().unwrap().remove(&id);
        let _ = app.emit("term-out", TermOut { id: &id, data: None, exit: true });
    });
    Ok(())
}

/// Keystrokes (and the chat's ▶ run button) go straight to the PTY.
#[tauri::command]
pub fn term_write(terms: State<'_, Terminals>, id: String, data: String) -> Result<(), String> {
    let mut map = terms.0.lock().unwrap();
    let handle = map.get_mut(&id).ok_or("terminal não existe")?;
    handle.writer.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
    handle.writer.flush().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn term_resize(terms: State<'_, Terminals>, id: String, cols: u16, rows: u16) {
    if let Some(handle) = terms.0.lock().unwrap().get(&id) {
        let _ = handle.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }
}

/// Close a tab: kill the shell. The reader thread notices EOF and cleans up.
#[tauri::command]
pub fn term_close(terms: State<'_, Terminals>, id: String) {
    if let Some(mut handle) = terms.0.lock().unwrap().remove(&id) {
        let _ = handle.child.kill();
    }
}

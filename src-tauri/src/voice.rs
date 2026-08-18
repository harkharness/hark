//! Voice as a GLOBAL layer: the HUD window, the active-context registry
//! and the utterance planner/executor. Windows are views; the spoken word
//! is interpreted globally and routed to the chat it belongs to.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use vox_core::config::Config;

/// What the user is looking at right now: the focused window reports its
/// project and focused task, so an unaddressed sentence has a sane
/// default target ("corrige o teste" = the task on screen).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActiveCtx {
    pub project_path: Option<String>,
    pub project_name: Option<String>,
    pub task_title: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Default)]
pub struct ActiveContext(pub Mutex<ActiveCtx>);

#[tauri::command]
pub fn set_active_context(
    state: tauri::State<'_, ActiveContext>,
    ctx: ActiveCtx,
) -> Result<(), String> {
    *state.0.lock().unwrap() = ctx;
    Ok(())
}

/// Where a planned utterance will land — shown on the HUD BEFORE anything
/// runs, so the wrong-session class of accidents dies at the chip.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VoicePlan {
    /// A local command (open project, recover session, board…): the HUD
    /// executes it straight away, zero tokens.
    Command { command: serde_json::Value },
    /// Real work with a resolved destination.
    Work {
        instruction: String,
        task_title: Option<String>,
        session_id: Option<String>,
        workspace: Option<String>,
        project_name: Option<String>,
        /// True = fresh session in the project (no task addressed).
        new_task: bool,
    },
    /// A question for the vox ask pipeline (speaks the answer).
    Question { question: String },
    /// Work with no resolvable destination: the HUD asks for an address.
    NoTarget { instruction: String },
}

/// Name of the registered project owning a path, plus its root.
fn project_of(config: &Config, path: &str) -> (Option<String>, Option<String>) {
    let projects = super::load_projects(config);
    let hit = projects
        .iter()
        .find(|p| path == p.path || path.starts_with(&format!("{}/", p.path)));
    match hit {
        Some(p) => (Some(p.name.clone()), Some(p.path.clone())),
        None => (None, Some(path.to_string())),
    }
}

/// Interpret one utterance GLOBALLY and say where it would land. Never
/// executes anything; pure planning over local data (index/board), zero
/// tokens.
#[tauri::command(async)]
pub fn plan_utterance(app: AppHandle, text: String) -> Result<VoicePlan, String> {
    let config = Config::load();
    let ctx = app.state::<ActiveContext>().0.lock().unwrap().clone();

    // 1. Local commands win (open project, recover session, rename…).
    if let Some(command) = super::task_command(text.clone(), ctx.task_title.clone())? {
        return Ok(VoicePlan::Command { command });
    }

    // 2. Explicit address: "na task X…", "no projeto Y…".
    let addr = vox_core::domain::address::parse(&text);
    if let Some(task_query) = &addr.task {
        // Board first (titles the user knows), then the whole index.
        let board_hit = super::with_board(|_, tasks| {
            Ok(vox_core::domain::board::find(&tasks, task_query))
        })?;
        if let Some(task) = board_hit {
            let (project_name, workspace) = task
                .workspace
                .as_deref()
                .map(|w| project_of(&config, w))
                .unwrap_or((None, None));
            return Ok(VoicePlan::Work {
                instruction: addr.instruction,
                session_id: task.session_ids.last().cloned(),
                task_title: Some(task.title),
                workspace,
                project_name,
                new_task: false,
            });
        }
        if let Some(hit) = super::session_hits(task_query, 1)?.into_iter().next() {
            let (project_name, workspace) = hit
                .cwd
                .as_deref()
                .map(|w| project_of(&config, w))
                .unwrap_or((None, None));
            return Ok(VoicePlan::Work {
                instruction: addr.instruction,
                session_id: Some(hit.session_id),
                task_title: Some(hit.title),
                workspace,
                project_name,
                new_task: false,
            });
        }
        return Ok(VoicePlan::NoTarget { instruction: text });
    }
    if let Some(project_query) = &addr.project {
        let projects = super::load_projects(&config);
        let hit = vox_core::domain::project::find(&projects, project_query)
            .or_else(|| vox_core::domain::project::find_spoken(&projects, project_query));
        return Ok(match hit {
            Some(p) => VoicePlan::Work {
                instruction: addr.instruction,
                session_id: None,
                task_title: None,
                workspace: Some(p.path.clone()),
                project_name: Some(p.name.clone()),
                new_task: true,
            },
            None => VoicePlan::NoTarget { instruction: text },
        });
    }

    // 3. No address: questions go to ask; work goes to the ACTIVE task
    //    (what the user is looking at), else to the best session match.
    if vox_core::domain::intent::route(&text) == vox_core::domain::intent::Route::Ask {
        return Ok(VoicePlan::Question { question: text });
    }
    if let (Some(title), Some(session)) = (&ctx.task_title, &ctx.session_id) {
        return Ok(VoicePlan::Work {
            instruction: text,
            task_title: Some(title.clone()),
            session_id: Some(session.clone()),
            workspace: ctx.project_path.clone(),
            project_name: ctx.project_name.clone(),
            new_task: false,
        });
    }
    if let Some(hit) = super::session_hits(&text, 1)?.into_iter().next() {
        let (project_name, workspace) = hit
            .cwd
            .as_deref()
            .map(|w| project_of(&config, w))
            .unwrap_or((None, None));
        return Ok(VoicePlan::Work {
            instruction: text,
            session_id: Some(hit.session_id),
            task_title: Some(hit.title),
            workspace,
            project_name,
            new_task: false,
        });
    }
    Ok(VoicePlan::NoTarget { instruction: text })
}

/// Execute a confirmed Work plan: live worker gets the message, dead
/// session gets a resume, new task gets a fresh chat — then the owning
/// project window opens/focuses on that task. Emits `voice_action` so the
/// mother's feed records what the voice did and where.
#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
pub fn voice_execute(
    app: AppHandle,
    instruction: String,
    session_id: Option<String>,
    workspace: Option<String>,
    project_name: Option<String>,
    task_title: Option<String>,
    new_task: bool,
) -> Result<serde_json::Value, String> {
    let out: serde_json::Value = if new_task || session_id.is_none() {
        let ws = workspace.clone().ok_or("nenhum projeto pra abrir o chat")?;
        let started = super::chat_start(app.clone(), app.state(), ws, instruction.clone(), None)?;
        serde_json::to_value(started).map_err(|e| e.to_string())?
    } else {
        let session = session_id.clone().unwrap();
        // A live worker already bound to this session takes the message
        // directly; otherwise resume the session in a fresh worker.
        let existing = {
            let live = app.state::<super::LiveWorkers>();
            let map = live.0.lock().unwrap();
            map.iter()
                .find(|(_, h)| h.spawn.session_id == session)
                .map(|(id, _)| id.clone())
        };
        match existing {
            Some(task_id) => {
                super::worker_send(
                    app.clone(),
                    app.state(),
                    task_id.clone(),
                    instruction.clone(),
                    None,
                )?;
                serde_json::json!({ "status": "sent", "task_id": task_id })
            }
            None => {
                let started = super::worker_start(
                    app.clone(),
                    app.state(),
                    app.state(),
                    instruction.clone(),
                    Some(session),
                    None,
                )?;
                serde_json::to_value(started).map_err(|e| e.to_string())?
            }
        }
    };

    // Bring the user to where the work landed.
    if let Some(ws) = &workspace {
        let name = project_name.clone().unwrap_or_else(|| {
            ws.split('/').rfind(|s| !s.is_empty()).unwrap_or("projeto").to_string()
        });
        let _ = super::open_project_window(
            app.clone(),
            name,
            ws.clone(),
            task_title.clone(),
            session_id.clone(),
        );
    }
    let _ = app.emit(
        "vox",
        serde_json::json!({
            "kind": "voice_action",
            "utterance": instruction,
            "target": task_title.or(project_name),
            "status": "despachado",
        }),
    );
    Ok(out)
}

/// Show (or create) the floating voice HUD: bottom-center, frameless,
/// always on top. Every show tells it to start listening.
pub fn show_hud(app: &AppHandle) {
    if let Some(hud) = app.get_webview_window("hud") {
        position_hud(app, &hud);
        let _ = hud.show();
        let _ = hud.set_focus();
        let _ = app.emit_to("hud", "vox", serde_json::json!({ "kind": "hud_listen" }));
        return;
    }
    let built = tauri::WebviewWindowBuilder::new(
        app,
        "hud",
        tauri::WebviewUrl::App("index.html?hud=1".into()),
    )
    .title("vox")
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .resizable(false)
    .skip_taskbar(true)
    .inner_size(620.0, 180.0)
    .build();
    if let Ok(hud) = built {
        position_hud(app, &hud);
        let _ = hud.set_focus();
        // The fresh webview starts listening on its own at mount.
    }
}

fn position_hud(app: &AppHandle, hud: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let w = (620.0 * scale) as i32;
        let h = (180.0 * scale) as i32;
        let x = monitor.position().x + ((size.width as i32 - w) / 2).max(0);
        let y = monitor.position().y + (size.height as i32 - h - (140.0 * scale) as i32).max(0);
        let _ = hud.set_position(tauri::PhysicalPosition { x, y });
    }
}

#[tauri::command]
pub fn hud_show(app: AppHandle) {
    show_hud(&app);
}

#[tauri::command]
pub fn hud_hide(app: AppHandle) {
    if let Some(hud) = app.get_webview_window("hud") {
        let _ = hud.hide();
    }
}

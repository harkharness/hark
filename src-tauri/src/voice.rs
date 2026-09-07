//! Voice as a GLOBAL layer: the HUD window, the active-context registry
//! and the utterance planner/executor. Windows are views; the spoken word
//! is interpreted globally and routed to the chat it belongs to.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use hark_core::config::Config;

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

/// Focus ledger keyed by window label: the spoken word targets the last
/// PROJECT window the user worked in. The mother/HUD focusing (or a
/// window closing) never erases that — see domain::focus for the rules.
#[derive(Default)]
pub struct ActiveContext(pub Mutex<hark_core::domain::focus::FocusLedger<ActiveCtx>>);

#[tauri::command]
pub fn set_active_context(
    window: tauri::Window,
    state: tauri::State<'_, ActiveContext>,
    ctx: ActiveCtx,
) -> Result<(), String> {
    state.0.lock().unwrap().report(window.label(), ctx);
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
        /// "high" = the chat on screen or a unique explicit address
        /// (silence confirms); "low" = resolved by search (the HUD must
        /// hear an explicit verdict before executing).
        confidence: String,
        /// Local precheck texts (heavy history, near-full context): any
        /// warning downgrades to an explicit verdict + offers actions.
        warnings: Vec<String>,
    },
    /// The target is ambiguous: real options for the user to pick from,
    /// best first — never a silent guess (the 19/08 incident).
    Candidates {
        instruction: String,
        options: Vec<VoiceCandidate>,
    },
    /// A question for the hark ask pipeline (speaks the answer).
    Question { question: String },
    /// Work with no resolvable destination: the HUD asks for an address.
    NoTarget { instruction: String },
    /// A clean spoken yes/no while a permission card waits ANYWHERE:
    /// hotkey + "pode" answers it without touching a window.
    PermissionAnswer {
        request_id: String,
        label: String,
        tool: String,
        allow: bool,
        /// Spoken "sempre pode": the HUD broadcasts an allow_rule event and
        /// the owning window records the standing rule in its own map.
        always: bool,
    },
}

/// One possible destination offered on the HUD.
#[derive(Serialize, Clone)]
pub struct VoiceCandidate {
    pub title: String,
    pub session_id: Option<String>,
    pub workspace: Option<String>,
    pub project_name: Option<String>,
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

/// Local precheck texts for a session-bound plan (zero tokens).
fn session_warnings(session_id: Option<&str>) -> Vec<String> {
    session_id
        .map(|sid| {
            hark_core::domain::precheck::prechecks(
                &super::session_facts(sid),
                hark_core::config::Config::load().lang(),
            )
                .into_iter()
                .map(|w| w.text)
                .collect()
        })
        .unwrap_or_default()
}

/// Interpret one utterance GLOBALLY and say where it would land. Never
/// executes anything; pure planning over local data (index/board), zero
/// tokens.
#[tauri::command(async)]
pub fn plan_utterance(app: AppHandle, text: String) -> Result<VoicePlan, String> {
    use hark_core::domain::funnel;
    let config = Config::load();
    let ctx = app
        .state::<ActiveContext>()
        .0
        .lock()
        .unwrap()
        .current()
        .cloned()
        .unwrap_or_default();

    // 0. A permission card waiting anywhere + a clean spoken verdict =
    //    the answer, whatever window it belongs to.
    let newest_perm = app.state::<super::PermLog>().0.lock().unwrap().last().cloned();
    if let Some((request_id, label, tool)) = newest_perm {
        if let Some((allow, always)) = hark_core::domain::verdict::interpret_permission(&text) {
            return Ok(VoicePlan::PermissionAnswer { request_id, label, tool, allow, always });
        }
    }

    // 1. Local commands win (open project, recover session, rename…).
    if let Some(command) = super::task_command(text.clone(), ctx.task_title.clone())? {
        // Compaction is not local: it is "/compact" delivered to the chat
        // on screen — the normal Work pipeline, confirm chip included.
        if command.get("kind").and_then(|k| k.as_str()) == Some("compact") {
            if ctx.session_id.is_some() {
                return Ok(VoicePlan::Work {
                    instruction: "/compact".into(),
                    task_title: ctx.task_title.clone(),
                    session_id: ctx.session_id.clone(),
                    workspace: ctx.project_path.clone(),
                    project_name: ctx.project_name.clone(),
                    new_task: false,
                    confidence: "high".into(),
                    // Compacting IS the remedy: no warning loop here.
                    warnings: vec![],
                });
            }
            return Ok(VoicePlan::NoTarget { instruction: text });
        }
        return Ok(VoicePlan::Command { command });
    }

    // 2-4. The pure funnel (domain::funnel): address → question → active
    //    chat → search. It lives in the core so the voice corpus replays
    //    real sentences through the SAME code this command runs.
    let board = super::with_board(|_, tasks| Ok(tasks))?;
    let projects = super::load_projects(&config);
    let search = |q: &str| {
        super::session_hits(q, 8)
            .unwrap_or_default()
            .into_iter()
            .map(|h| funnel::SessionLead {
                session_id: h.session_id,
                title: h.title,
                cwd: h.cwd,
                last_ts: h.last_ts,
                last_prompt: h.last_prompt,
            })
            .collect()
    };
    let world = funnel::World {
        active: match (&ctx.task_title, &ctx.session_id) {
            (Some(t), Some(s)) => Some((t.as_str(), s.as_str())),
            _ => None,
        },
        active_project: match (&ctx.project_name, &ctx.project_path) {
            (Some(n), Some(p)) => Some((n.as_str(), p.as_str())),
            _ => None,
        },
        board: &board,
        projects: &projects,
        search: &search,
    };
    Ok(match funnel::plan(&text, &world) {
        funnel::Plan::Question { question } => VoicePlan::Question { question },
        funnel::Plan::Work {
            instruction,
            task_title,
            session_id,
            workspace,
            new_task,
            confidence,
        } => {
            let (project_name, workspace) = workspace
                .as_deref()
                .map(|w| project_of(&config, w))
                .unwrap_or((None, None));
            VoicePlan::Work {
                warnings: session_warnings(session_id.as_deref()),
                instruction,
                task_title,
                session_id,
                workspace,
                project_name,
                new_task,
                confidence: confidence.to_string(),
            }
        }
        funnel::Plan::Candidates { instruction, options } => VoicePlan::Candidates {
            instruction,
            options: options
                .into_iter()
                .map(|c| {
                    let (project_name, workspace) = c
                        .workspace
                        .as_deref()
                        .map(|w| project_of(&config, w))
                        .unwrap_or((None, None));
                    VoiceCandidate { title: c.title, session_id: c.session_id, workspace, project_name }
                })
                .collect(),
        },
        funnel::Plan::NoTarget { instruction } => VoicePlan::NoTarget { instruction },
    })
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
    compact_first: Option<bool>,
) -> Result<serde_json::Value, String> {
    let compact_first = compact_first.unwrap_or(false);
    // "…o que decidimos no chat do X": the cited chat's lines ride inside
    // the spoken instruction too — same zero-token pull the typed path
    // does, announced in the feed so the injection is never invisible.
    let instruction = match super::crossref_lookup(instruction.clone()) {
        Ok(Some(ctx)) => {
            let _ = app.emit(
                "hark",
                serde_json::json!({ "kind": "status",
                    "text": format!("puxei {} linha(s) do chat \"{}\"", ctx.lines, ctx.title) }),
            );
            format!("{instruction}\n\n{}", ctx.block)
        }
        _ => instruction,
    };
    let out: serde_json::Value = if new_task || session_id.is_none() {
        let ws = workspace.clone().ok_or("nenhum projeto pra abrir o chat")?;
        let started =
            super::chat_start(app.clone(), app.state(), ws, instruction.clone(), None, None, None)?;
        serde_json::to_value(started).map_err(|e| e.to_string())?
    } else {
        let session = session_id.clone().unwrap();
        // A live worker already bound to this session takes the message
        // directly; otherwise resume the session in a fresh worker.
        let existing = {
            let live = app.state::<super::LiveWorkers>();
            let map = live.0.lock().unwrap();
            map.iter()
                .find(|(_, h)| h.spec.session_id == session)
                .map(|(id, _)| id.clone())
        };
        match existing {
            Some(task_id) => {
                // "compacta antes": /compact runs as its own turn, the
                // message queues right behind it on the worker's stdin.
                if compact_first {
                    super::worker_send(
                        app.clone(),
                        app.state(),
                        task_id.clone(),
                        "/compact".into(),
                        None,
                    )?;
                }
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
                // Resuming: the first message opens the worker (a resume
                // inherits the session's title, so "/compact" never
                // becomes a card name), the real work queues after it.
                let first = if compact_first { "/compact".to_string() } else { instruction.clone() };
                let started = super::worker_start(
                    app.clone(),
                    app.state(),
                    app.state(),
                    first,
                    Some(session),
                    None,
                    None,
                    None,
                    None,
                )?;
                let value = serde_json::to_value(started).map_err(|e| e.to_string())?;
                if compact_first {
                    if let Some(task_id) = value.get("task_id").and_then(|v| v.as_str()) {
                        super::worker_send(
                            app.clone(),
                            app.state(),
                            task_id.to_string(),
                            instruction.clone(),
                            None,
                        )?;
                    }
                }
                value
            }
        }
    };

    // Bring the user to where the work landed. A plan without a workspace
    // (board card never linked, bare session hit) still fronts a window:
    // the index knows the session's cwd.
    let workspace = workspace.or_else(|| {
        session_id.as_deref().and_then(|sid| {
            let config = Config::load();
            let store = hark_core::adapters::sqlite_store::SqliteStore::open(
                &config.data_dir().join("index.db"),
            )
            .ok()?;
            super::session_summary(&store, sid)?.cwd
        })
    });
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
        "hark",
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
        let _ = app.emit_to("hud", "hark", serde_json::json!({ "kind": "hud_listen" }));
        return;
    }
    let built = tauri::WebviewWindowBuilder::new(
        app,
        "hud",
        tauri::WebviewUrl::App("index.html?hud=1".into()),
    )
    .title("hark")
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

//! Tauri driver: the desktop window over the same vox-core used by the CLI.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::{mpsc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager, State};
use vox_core::adapters::claude_cli::ClaudeCli;
use vox_core::adapters::git_collect::GitCli;
use vox_core::adapters::live_sessions::ClaudeAgentsCli;
use vox_core::adapters::memory_files::{self, VoxDir};
use vox_core::adapters::sqlite_store::SqliteStore;
use vox_core::adapters::state_file;
use vox_core::adapters::{cpal_audio::CpalMic, say_tts::SayTts, whisper_stt::WhisperStt};
use vox_core::app::ask::{ask_with_image, AskDeps};
use vox_core::app::dispatch::{plan, Plan};
use vox_core::chrono::{SecondsFormat, Utc};
use vox_core::config::Config;
use vox_core::domain::claude_event::{ClaudeEvent, PermissionDecision, VoiceReply};
use vox_core::domain::memory::{WorkerRecord, WorkerStatus};
use vox_core::ports::{AgentRunner, AudioIn, Stt, Tts};

/// Permission requests waiting for a click, keyed by request_id.
struct Pending(Mutex<HashMap<String, mpsc::Sender<PermissionDecision>>>);

/// A live worker plus everything needed to restart it on the same session.
struct WorkerHandle {
    worker: std::sync::Arc<vox_core::adapters::worker::PersistentWorker>,
    spawn: vox_core::adapters::worker::WorkerSpawn,
}

/// Conversational workers still alive, keyed by task_id.
struct LiveWorkers(Mutex<HashMap<String, std::sync::Arc<WorkerHandle>>>);

/// Permission requests raised by live workers: request_id -> task_id.
struct WorkerPermissions(Mutex<HashMap<String, String>>);

/// Whisper loads once per process (heavy); everything else is per-call.
static STT: OnceLock<anyhow::Result<WhisperStt>> = OnceLock::new();

fn stt(config: &Config) -> Option<&'static WhisperStt> {
    STT.get_or_init(|| {
        let stt = WhisperStt::load(&config.whisper_model_path(), &config.language, &config.vocab)?;
        stt.warmup();
        Ok(stt)
    })
    .as_ref()
    .ok()
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn emit_event(app: &AppHandle, payload: serde_json::Value) {
    let _ = app.emit("vox", payload);
}

fn build_deps<'a>(
    config: &'a Config,
    store: &'a mut SqliteStore,
    live: &'a ClaudeAgentsCli,
    runner: &'a dyn AgentRunner,
    active_project: Option<vox_core::domain::project::Project>,
) -> AskDeps<'a> {
    let state = state_file::load(&config.data_dir());
    AskDeps {
        active_context: state.active_context,
        projects: state.projects,
        active_project,
        workers: state.workers,
        journal: &VoxDir,
        store,
        live,
        repos: &GitCli,
        runner,
        config,
    }
}

struct NoopRunner;
impl AgentRunner for NoopRunner {
    fn ask(
        &self,
        _request: &vox_core::ports::TurnRequest,
        _e: &mut dyn FnMut(&ClaudeEvent),
    ) -> anyhow::Result<vox_core::domain::claude_event::TurnResult> {
        anyhow::bail!("not used")
    }
}

#[derive(Serialize)]
struct Overview {
    contexts: Vec<String>,
    active: String,
    workers: Vec<WorkerRecord>,
    board: Vec<vox_core::domain::board::Task>,
    projects: Vec<vox_core::domain::project::Project>,
    theme: String,
}

/// The editable project list. First run seeds it from what the machine
/// already knows: board workspaces and configured context repos.
fn load_projects(config: &Config) -> Vec<vox_core::domain::project::Project> {
    use vox_core::domain::project;
    use vox_core::ports::SessionStore;
    let mut state = state_file::load(&config.data_dir());
    if !state.projects.is_empty() {
        return state.projects;
    }
    let mut candidates = std::collections::BTreeSet::new();
    if let Ok(store) = SqliteStore::open(&config.data_dir().join("index.db")) {
        for task in store.board().unwrap_or_default() {
            if let Some(workspace) = task.workspace {
                candidates.insert(workspace);
            }
        }
    }
    for name in config.context_names() {
        if let Some(ctx) = config.context(&name) {
            candidates.extend(ctx.repos);
        }
    }
    let mut projects = Vec::new();
    for path in candidates {
        if std::path::Path::new(&path).is_dir() {
            let (next, _) = project::add(projects, &project::derive_name(&path), &path);
            projects = next;
        }
    }
    state.projects = projects.clone();
    let _ = state_file::save(&config.data_dir(), &state);
    projects
}

#[tauri::command]
fn overview() -> Overview {
    use vox_core::ports::SessionStore;
    let config = Config::load();
    let state = state_file::load(&config.data_dir());
    let mut workers = state.workers;
    workers.reverse(); // newest first
    workers.truncate(12);
    let mut board = SqliteStore::open(&config.data_dir().join("index.db"))
        .and_then(|s| s.board())
        .unwrap_or_default();
    board.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Overview {
        contexts: config.context_names(),
        active: state
            .active_context
            .unwrap_or_else(|| config.default_context.clone()),
        workers,
        board,
        projects: load_projects(&config),
        theme: config.theme,
    }
}

#[tauri::command]
fn project_add(path: String) -> Result<vox_core::domain::project::Project, String> {
    use vox_core::domain::project;
    let expanded = vox_core::config::expand_home(&path);
    let root = std::path::PathBuf::from(&expanded)
        .canonicalize()
        .map_err(|_| format!("diretório não existe: {expanded}"))?;
    if !root.is_dir() {
        return Err(format!("não é um diretório: {}", root.display()));
    }
    let config = Config::load();
    let path_str = root.display().to_string();
    let mut state = state_file::load(&config.data_dir());
    let seeded = load_projects(&config);
    let (projects, entry) = project::add(seeded, &project::derive_name(&path_str), &path_str);
    state.projects = projects;
    state_file::save(&config.data_dir(), &state).map_err(|e| e.to_string())?;
    Ok(entry)
}

#[tauri::command]
fn project_remove(key: String) -> Result<(), String> {
    let config = Config::load();
    let mut state = state_file::load(&config.data_dir());
    state.projects = vox_core::domain::project::remove(state.projects, &key);
    state_file::save(&config.data_dir(), &state).map_err(|e| e.to_string())
}

/// Fuzzy file search inside a project (drives @mention and quick-open).
#[tauri::command(async)]
fn project_files(path: String, query: String, limit: Option<usize>) -> Vec<String> {
    let root = std::path::PathBuf::from(vox_core::config::expand_home(&path));
    let files = vox_core::adapters::fs_files::list_files(&root, 8000);
    vox_core::domain::file_search::search(
        &query,
        files.iter().map(|s| s.as_str()),
        limit.unwrap_or(30),
    )
}

/// Only files inside registered projects are readable/writable from the UI.
fn guard_project_path(path: &str) -> Result<std::path::PathBuf, String> {
    let config = Config::load();
    let target = std::path::PathBuf::from(vox_core::config::expand_home(path))
        .canonicalize()
        .map_err(|e| format!("{path}: {e}"))?;
    let allowed = load_projects(&config).iter().any(|proj| {
        std::path::PathBuf::from(vox_core::config::expand_home(&proj.path))
            .canonicalize()
            .map(|root| target.starts_with(root))
            .unwrap_or(false)
    });
    if allowed {
        Ok(target)
    } else {
        Err("arquivo fora dos projetos registrados".into())
    }
}

#[derive(Serialize)]
struct FileOut {
    content: String,
    truncated: bool,
}

/// Read a file for the local viewer. Zero tokens: never touches Claude.
#[tauri::command(async)]
fn file_read(path: String) -> Result<FileOut, String> {
    let target = guard_project_path(&path)?;
    let content = std::fs::read_to_string(&target).map_err(|e| e.to_string())?;
    const MAX: usize = 400_000;
    if content.chars().count() > MAX {
        Ok(FileOut {
            content: content.chars().take(MAX).collect(),
            truncated: true,
        })
    } else {
        Ok(FileOut {
            content,
            truncated: false,
        })
    }
}

/// Save a local edit made by the USER in the viewer (never by a model).
#[tauri::command(async)]
fn file_save(path: String, content: String) -> Result<(), String> {
    let target = guard_project_path(&path)?;
    std::fs::write(&target, content).map_err(|e| e.to_string())
}

#[tauri::command]
fn use_context(name: String) -> Result<(), String> {
    let config = Config::load();
    if name != "all" && config.context(&name).is_none() {
        return Err(format!("unknown context {name}"));
    }
    let state = state_file::GlobalState {
        active_context: (name != "all").then_some(name),
        ..state_file::load(&config.data_dir())
    };
    state_file::save(&config.data_dir(), &state).map_err(|e| e.to_string())
}

#[tauri::command]
fn route_text(text: String) -> String {
    match vox_core::domain::intent::route(&text) {
        vox_core::domain::intent::Route::Dispatch => "dispatch".into(),
        vox_core::domain::intent::Route::Ask => "ask".into(),
    }
}

#[tauri::command]
fn speak(app: AppHandle, text: String) {
    let config = Config::load();
    std::thread::spawn(move || {
        // The voice orb follows these events (no audio analysis needed).
        emit_event(&app, serde_json::json!({ "kind": "speaking", "on": true }));
        let _ = SayTts {
            voice: config.voice,
        }
        .speak(&text);
        emit_event(&app, serde_json::json!({ "kind": "speaking", "on": false }));
    });
}

/// Cut any in-flight TTS immediately (Esc in the window).
#[tauri::command]
fn speak_stop(app: AppHandle) {
    let _ = std::process::Command::new("killall").arg("say").status();
    emit_event(&app, serde_json::json!({ "kind": "speaking", "on": false }));
}

/// Shared manual-cut flag: `hear_stop` (Esc) flips it, the capture loop in
/// `hear_once` sees it and returns what was said so far.
fn mic_stop_flag() -> std::sync::Arc<std::sync::atomic::AtomicBool> {
    static FLAG: std::sync::OnceLock<std::sync::Arc<std::sync::atomic::AtomicBool>> =
        std::sync::OnceLock::new();
    FLAG.get_or_init(|| std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)))
        .clone()
}

#[tauri::command(async)]
fn hear_once() -> Result<String, String> {
    let config = Config::load();
    let stt = stt(&config).ok_or("whisper model missing (run: vox setup)")?;
    let tts = SayTts {
        voice: config.voice.clone(),
    };
    let stop = mic_stop_flag();
    stop.store(false, std::sync::atomic::Ordering::SeqCst);
    tts.beep(vox_core::ports::Cue::Listening);
    let audio = CpalMic {
        stop,
        ..CpalMic::default()
    }
    .record_utterance()
    .map_err(|e| e.to_string())?;
    tts.beep(vox_core::ports::Cue::Captured);
    stt.transcribe(&audio).map_err(|e| e.to_string())
}

/// Esc while the orb is red: cut the capture NOW and transcribe what was
/// already said (the VAD can be slow in a noisy room).
#[tauri::command]
fn hear_stop() {
    mic_stop_flag().store(true, std::sync::atomic::Ordering::SeqCst);
}

#[derive(Serialize)]
struct ReplyOut {
    fala: String,
    detalhes: String,
    itens: Vec<String>,
    cost_usd: Option<f64>,
    model: Option<String>,
}

#[tauri::command(async)]
fn ask_text(
    app: AppHandle,
    question: String,
    image_b64: Option<String>,
    media_type: Option<String>,
    project_name: Option<String>,
    project_path: Option<String>,
) -> Result<ReplyOut, String> {
    let config = Config::load();
    let mut store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let live = ClaudeAgentsCli {
        claude_bin: config.claude_bin_resolved(),
    };
    let runner = ClaudeCli {
        claude_bin: config.claude_bin_resolved(),

        work_dir: config.data_dir(),
    };
    // The UI's focused project scopes this question (click = context).
    let active_project = match (project_name, project_path) {
        (Some(name), Some(path)) => Some(vox_core::domain::project::Project { name, path }),
        _ => None,
    };
    let mut deps = build_deps(&config, &mut store, &live, &runner, active_project);
    let image = match (&media_type, &image_b64) {
        (Some(m), Some(d)) => Some((m.as_str(), d.as_str())),
        _ => None,
    };
    let result = ask_with_image(&question, image, &mut deps, &mut |event| {
        if let ClaudeEvent::ToolUse { name, input } = event {
            if name != "StructuredOutput" {
                emit_event(
                    &app,
                    serde_json::json!({ "kind": "tool", "name": name, "input": input }),
                );
            }
        }
    })
    .map_err(|e| format!("{e:#}"))?;

    match result.reply {
        Some(VoiceReply { fala, detalhes, itens, .. }) => Ok(ReplyOut {
            fala,
            detalhes,
            itens,
            cost_usd: result.cost_usd,
            model: result.model,
        }),
        None if !result.is_error => Ok(ReplyOut {
            fala: result.raw.clone(),
            detalhes: result.raw,
            itens: vec![],
            cost_usd: result.cost_usd,
            model: result.model,
        }),
        None => Err(result.raw),
    }
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum DispatchOut {
    /// Conversational worker is up; results arrive as events.
    Started {
        task_id: String,
        directives: vox_core::domain::directives::Directives,
    },
    Done {
        task_id: String,
        summary: String,
        cost_usd: Option<f64>,
    },
    Failed {
        task_id: String,
        summary: String,
        /// Even failed turns burn tokens; the UI shows it.
        cost_usd: Option<f64>,
    },
    Choice {
        candidates: Vec<Candidate>,
    },
    Busy {
        session_id: String,
    },
    NoMatch,
}

#[derive(Serialize)]
struct Candidate {
    session_id: String,
    title: String,
    last_ts: String,
}

/// Start a CONVERSATIONAL worker: plans the target like dispatch, spawns a
/// persistent claude process, streams rich events, and stays alive for
/// follow-up messages via worker_send. Returns as soon as it starts.
#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
fn worker_start(
    app: AppHandle,
    live: State<'_, LiveWorkers>,
    perms: State<'_, WorkerPermissions>,
    instruction: String,
    session_id: Option<String>,
) -> Result<DispatchOut, String> {
    let config = Config::load();
    let planned = {
        let mut store =
            SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
        let agents = ClaudeAgentsCli {
            claude_bin: config.claude_bin_resolved(),
        };
        let mut deps = build_deps(&config, &mut store, &agents, &NoopRunner, None);
        plan(&mut deps, &instruction, session_id.as_deref()).map_err(|e| e.to_string())?
    };
    let planned = match planned {
        Plan::Ready(p) => p,
        Plan::NeedsChoice(candidates) => {
            return Ok(DispatchOut::Choice {
                candidates: candidates
                    .into_iter()
                    .map(|c| Candidate {
                        title: c
                            .title
                            .or(c.last_prompt)
                            .unwrap_or_else(|| c.session_id.clone()),
                        session_id: c.session_id,
                        last_ts: c.last_ts.unwrap_or_default(),
                    })
                    .collect(),
            })
        }
        Plan::TargetBusy(s) => {
            return Ok(DispatchOut::Busy {
                session_id: s.session_id,
            })
        }
        Plan::NoMatch => return Ok(DispatchOut::NoMatch),
    };

    let task_id = format!(
        "t-{}-{}",
        &planned.session.session_id[..6.min(planned.session.session_id.len())],
        Utc::now().format("%m%d%H%M%S")
    );
    let brief = format!(
        "# Vox dispatch {task_id}\n\n- session: {}\n- workspace: {}\n\n## Instruction\n\n{instruction}\n",
        planned.session.session_id,
        planned.workspace_root.display()
    );
    let _ = memory_files::write_brief(&planned.workspace_root, &task_id, &brief);
    update_registry_and_board(
        &config,
        &task_id,
        &planned.workspace_root,
        Some(&planned.session.session_id),
        &instruction,
        WorkerStatus::Running,
    );

    let directives = vox_core::domain::directives::parse(&instruction);
    let spawn = vox_core::adapters::worker::WorkerSpawn {
        limits: config.spawn_limits(),
        claude_bin: config.claude_bin_resolved(),
        cwd: planned.workspace_root.clone(),
        session_id: planned.session.session_id.clone(),
        instruction: instruction.clone(),
        directives: directives.clone(),
    };
    let _ = &perms; // permissions are looked up via app.state in the reader
    start_worker(&app, &live, &task_id, spawn).map_err(|e| e.to_string())?;
    Ok(DispatchOut::Started {
        task_id,
        directives,
    })
}

/// Spawn a worker process for `task_id` and pump its output into the UI.
/// Used by the first dispatch and by every restart (directive change).
fn start_worker(
    app: &AppHandle,
    live: &State<'_, LiveWorkers>,
    task_id: &str,
    spawn: vox_core::adapters::worker::WorkerSpawn,
) -> anyhow::Result<()> {
    let (worker, stdout) = vox_core::adapters::worker::PersistentWorker::spawn(&spawn)?;
    let pid = worker.pid;
    live.0.lock().unwrap().insert(
        task_id.to_string(),
        std::sync::Arc::new(WorkerHandle {
            worker: std::sync::Arc::new(worker),
            spawn: spawn.clone(),
        }),
    );

    // Reader loop: parse every stdout line, stream rich events to the UI,
    // route permission requests, keep going across turns until EOF.
    let app2 = app.clone();
    let task2 = task_id.to_string();
    let workspace = spawn.cwd.clone();
    let is_new_session = spawn.session_id.is_empty();
    let board_title: String = spawn.instruction.chars().take(60).collect();
    let mut current_session = spawn.session_id.clone();
    std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        let config = Config::load();
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            match vox_core::domain::claude_event::parse(&line) {
                ClaudeEvent::SessionStarted(session_id) => {
                    current_session = session_id.clone();
                    if !is_new_session {
                        continue;
                    }
                    // A fresh session finally has an id: link registry+board.
                    let mut gstate = state_file::load(&config.data_dir());
                    if let Some(w) = gstate.workers.iter_mut().find(|w| w.task_id == task2) {
                        w.session_id = session_id.clone();
                    }
                    let _ = state_file::save(&config.data_dir(), &gstate);
                    use vox_core::ports::SessionStore;
                    if let Ok(mut store) = SqliteStore::open(&config.data_dir().join("index.db")) {
                        if let Ok(current) = store.board() {
                            let updates = [vox_core::domain::board::BoardUpdate {
                                titulo: board_title.clone(),
                                status: vox_core::domain::board::TaskStatus::Doing,
                                nota: None,
                                sessao: None,
                            }];
                            let merged = vox_core::domain::board::apply_updates(
                                current,
                                &updates,
                                &now_iso(),
                                Some(&workspace.display().to_string()),
                                Some(&session_id),
                            );
                            let _ = store.save_board(&merged);
                        }
                    }
                    emit_event(
                        &app2,
                        serde_json::json!({ "kind": "session_started",
                            "task_id": task2, "session_id": session_id }),
                    );
                }
                ClaudeEvent::AssistantText(text) => emit_event(
                    &app2,
                    serde_json::json!({ "kind": "assistant_text", "task_id": task2, "text": text }),
                ),
                ClaudeEvent::ToolUse { name, input } => emit_event(
                    &app2,
                    serde_json::json!({ "kind": "worker", "task_id": task2, "name": name, "input": input }),
                ),
                ClaudeEvent::ToolResult { content, is_error } => emit_event(
                    &app2,
                    serde_json::json!({ "kind": "tool_result", "task_id": task2,
                        "content": content.chars().take(4000).collect::<String>(), "is_error": is_error }),
                ),
                ClaudeEvent::PermissionRequest {
                    request_id,
                    tool_name,
                    input,
                } => {
                    app2.state::<WorkerPermissions>()
                        .0
                        .lock()
                        .unwrap()
                        .insert(request_id.clone(), task2.clone());
                    let _ = app2.emit(
                        "vox-permission",
                        serde_json::json!({
                            "request_id": request_id,
                            "task_id": task2,
                            "tool_name": tool_name,
                            "input": input,
                        }),
                    );
                }
                ClaudeEvent::Result(turn) => {
                    // Turn done, worker stays alive for the next message.
                    update_worker_summary(&config, &task2, &turn.raw);
                    let _ = memory_files::append_state(
                        &workspace,
                        &format!("- {} {task2} [turn] {}", now_iso(), turn.raw.chars().take(160).collect::<String>()),
                    );
                    let workspace_str = workspace.display().to_string();
                    record_live_spend(
                        &config,
                        vox_core::domain::spend::SpendKind::Worker,
                        &vox_core::domain::spend::SpendMeta {
                            task_id: Some(&task2),
                            session_id: (!current_session.is_empty())
                                .then_some(current_session.as_str()),
                            workspace: Some(&workspace_str),
                            ..Default::default()
                        },
                        &turn,
                    );
                    // Aggregate usage + how full the context window is.
                    let mut usage = vox_core::domain::claude_event::TokenUsage::default();
                    let mut window: Option<u64> = None;
                    for m in &turn.usage {
                        usage.input += m.usage.input;
                        usage.output += m.usage.output;
                        usage.cache_read += m.usage.cache_read;
                        usage.cache_created += m.usage.cache_created;
                        if m.context_window.unwrap_or(0) > window.unwrap_or(0) {
                            window = m.context_window;
                        }
                    }
                    let context_pct = window.filter(|w| *w > 0).map(|w| {
                        ((usage.input + usage.cache_read + usage.cache_created) as f64
                            / w as f64)
                            .min(1.0)
                    });
                    emit_event(
                        &app2,
                        serde_json::json!({ "kind": "worker_turn", "task_id": task2,
                            "text": turn.raw, "cost_usd": turn.cost_usd, "model": turn.model, "is_error": turn.is_error,
                            "usage": { "input": usage.input, "output": usage.output,
                                       "cache_read": usage.cache_read, "cache_created": usage.cache_created },
                            "context_pct": context_pct }),
                    );
                }
                ClaudeEvent::RateLimit(info) => emit_event(
                    &app2,
                    serde_json::json!({ "kind": "rate_limit", "status": info.status,
                        "resets_at": info.resets_at, "limit_kind": info.kind }),
                ),
                _ => {}
            }
        }
        // Only report the exit if nobody restarted this task meanwhile.
        let restarted = app2
            .state::<LiveWorkers>()
            .0
            .lock()
            .unwrap()
            .get(&task2)
            .is_some_and(|h| h.worker.pid != pid);
        if !restarted {
            emit_event(
                &app2,
                serde_json::json!({ "kind": "worker_exit", "task_id": task2 }),
            );
            app2.state::<LiveWorkers>().0.lock().unwrap().remove(&task2);
        }
    });

    Ok(())
}

fn update_registry_and_board(
    config: &Config,
    task_id: &str,
    workspace_root: &std::path::Path,
    session_id: Option<&str>,
    instruction: &str,
    status: WorkerStatus,
) {
    let mut gstate = state_file::load(&config.data_dir());
    gstate.workers.push(WorkerRecord {
        task_id: task_id.to_string(),
        context: String::new(),
        workspace: workspace_root.display().to_string(),
        session_id: session_id.unwrap_or_default().to_string(),
        status,
        started_at: now_iso(),
        summary: instruction.chars().take(120).collect(),
    });
    let _ = state_file::save(&config.data_dir(), &gstate);

    use vox_core::ports::SessionStore;
    if let Ok(mut store) = SqliteStore::open(&config.data_dir().join("index.db")) {
        if let Ok(current) = store.board() {
            let updates = [vox_core::domain::board::BoardUpdate {
                titulo: instruction.chars().take(60).collect(),
                status: vox_core::domain::board::TaskStatus::Doing,
                nota: Some("worker conversacional ativo".into()),
                sessao: None,
            }];
            let merged = vox_core::domain::board::apply_updates(
                current,
                &updates,
                &now_iso(),
                Some(&workspace_root.display().to_string()),
                session_id,
            );
            let _ = store.save_board(&merged);
        }
    }
}

/// Brand-new chat (fresh Claude Code session) inside a project directory.
/// No plan/resume: the session id arrives via `SessionStarted` and is then
/// linked to the board task and registry.
#[tauri::command(async)]
fn chat_start(
    app: AppHandle,
    live: State<'_, LiveWorkers>,
    project_path: String,
    instruction: String,
) -> Result<DispatchOut, String> {
    let config = Config::load();
    let root = std::path::PathBuf::from(vox_core::config::expand_home(&project_path));
    if !root.is_dir() {
        return Err(format!("diretório não existe: {}", root.display()));
    }
    let task_id = format!("n-{}", Utc::now().format("%m%d%H%M%S"));
    update_registry_and_board(&config, &task_id, &root, None, &instruction, WorkerStatus::Running);

    let directives = vox_core::domain::directives::parse(&instruction);
    let spawn = vox_core::adapters::worker::WorkerSpawn {
        limits: config.spawn_limits(),
        claude_bin: config.claude_bin_resolved(),
        cwd: root,
        session_id: String::new(),
        instruction,
        directives: directives.clone(),
    };
    start_worker(&app, &live, &task_id, spawn).map_err(|e| e.to_string())?;
    Ok(DispatchOut::Started { task_id, directives })
}

/// Persist one live turn into the spend ledger. Failures never break the
/// flow: the ledger is bookkeeping, not the critical path.
fn record_live_spend(
    config: &Config,
    kind: vox_core::domain::spend::SpendKind,
    meta: &vox_core::domain::spend::SpendMeta,
    turn: &vox_core::domain::claude_event::TurnResult,
) {
    use vox_core::ports::SpendLedger;
    let rows = vox_core::domain::spend::rows_from_turn(&now_iso(), kind, meta, turn);
    if rows.is_empty() {
        return;
    }
    if let Ok(mut store) = SqliteStore::open(&config.data_dir().join("index.db")) {
        let _ = store.record_spend(&rows);
    }
}

fn update_worker_summary(config: &Config, task_id: &str, summary: &str) {
    let mut gstate = state_file::load(&config.data_dir());
    if let Some(record) = gstate.workers.iter_mut().find(|w| w.task_id == task_id) {
        record.summary = summary.chars().take(200).collect();
    }
    let _ = state_file::save(&config.data_dir(), &gstate);
}

/// Follow-up message into a live worker. If the message carries session
/// directives ("planeja isso", "usa o opus", "capricha"), the worker is
/// restarted on the SAME session with the new flags, so the conversation
/// continues with the requested mode/effort/model.
#[tauri::command(async)]
fn worker_send(
    app: AppHandle,
    live: State<'_, LiveWorkers>,
    task_id: String,
    text: String,
    image_b64: Option<String>,
    media_type: Option<String>,
) -> Result<vox_core::domain::directives::Directives, String> {
    let handle = live
        .0
        .lock()
        .unwrap()
        .get(&task_id)
        .cloned()
        .ok_or("worker não está mais ativo")?;

    let asked = vox_core::domain::directives::parse(&text);
    let mut next = handle.spawn.directives.clone();
    if asked.mode.is_some() {
        next.mode = asked.mode;
    }
    if asked.effort.is_some() {
        next.effort = asked.effort;
    }
    if asked.model.is_some() {
        next.model = asked.model.clone();
    }

    if next == handle.spawn.directives {
        let image = match (&media_type, &image_b64) {
            (Some(m), Some(d)) => Some((m.as_str(), d.as_str())),
            _ => None,
        };
        handle
            .worker
            .send_text(&text, image)
            .map_err(|e| e.to_string())?;
        return Ok(next);
    }

    // Directive changed: only a fresh process can carry new CLI flags.
    emit_event(
        &app,
        serde_json::json!({ "kind": "status",
            "text": format!("reabrindo a thread com {}", describe(&next)) }),
    );
    handle.worker.shutdown();
    // Limits carry over from the original spawn (`..clone()`).
    let spawn = vox_core::adapters::worker::WorkerSpawn {
        instruction: text,
        directives: next.clone(),
        ..handle.spawn.clone()
    };
    start_worker(&app, &live, &task_id, spawn).map_err(|e| e.to_string())?;
    Ok(next)
}

fn describe(d: &vox_core::domain::directives::Directives) -> String {
    [
        d.mode.map(|m| format!("modo {}", m.label())),
        d.effort.map(|e| format!("esforço {}", e.as_flag())),
        d.model.clone(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ")
}

/// End the conversation: EOF + kill, board moves to waiting.
#[tauri::command]
fn worker_stop(live: State<'_, LiveWorkers>, task_id: String) -> Result<(), String> {
    let worker = live.0.lock().unwrap().remove(&task_id).ok_or("worker não encontrado")?;
    worker.worker.shutdown();
    let config = Config::load();
    let mut gstate = state_file::load(&config.data_dir());
    if let Some(record) = gstate.workers.iter_mut().find(|w| w.task_id == task_id) {
        record.status = WorkerStatus::Done;
    }
    let _ = state_file::save(&config.data_dir(), &gstate);
    Ok(())
}

#[tauri::command(async)]
fn dispatch_text(
    app: AppHandle,
    pending: State<'_, Pending>,
    instruction: String,
    session_id: Option<String>,
) -> Result<DispatchOut, String> {
    let config = Config::load();
    let planned = {
        let mut store =
            SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
        let live = ClaudeAgentsCli {
            claude_bin: config.claude_bin_resolved(),
        };
        let mut deps = build_deps(&config, &mut store, &live, &NoopRunner, None);
        plan(&mut deps, &instruction, session_id.as_deref()).map_err(|e| e.to_string())?
    };

    let planned = match planned {
        Plan::Ready(p) => p,
        Plan::NeedsChoice(candidates) => {
            return Ok(DispatchOut::Choice {
                candidates: candidates
                    .into_iter()
                    .map(|c| Candidate {
                        title: c
                            .title
                            .or(c.last_prompt)
                            .unwrap_or_else(|| c.session_id.clone()),
                        session_id: c.session_id,
                        last_ts: c.last_ts.unwrap_or_default(),
                    })
                    .collect(),
            })
        }
        Plan::TargetBusy(s) => {
            return Ok(DispatchOut::Busy {
                session_id: s.session_id,
            })
        }
        Plan::NoMatch => return Ok(DispatchOut::NoMatch),
    };

    let task_id = format!(
        "t-{}-{}",
        &planned.session.session_id[..6.min(planned.session.session_id.len())],
        Utc::now().format("%m%d%H%M%S")
    );
    emit_event(
        &app,
        serde_json::json!({ "kind": "status", "text": format!("{task_id}: retomando {} em {}", planned.session.session_id, planned.workspace_root.display()) }),
    );

    let brief = format!(
        "# Vox dispatch {task_id}\n\n- session: {}\n- workspace: {}\n\n## Instruction\n\n{instruction}\n",
        planned.session.session_id,
        planned.workspace_root.display()
    );
    let _ = memory_files::write_brief(&planned.workspace_root, &task_id, &brief);

    let mut gstate = state_file::load(&config.data_dir());
    gstate.workers.push(WorkerRecord {
        task_id: task_id.clone(),
        context: String::new(),
        workspace: planned.workspace_root.display().to_string(),
        session_id: planned.session.session_id.clone(),
        status: WorkerStatus::Running,
        started_at: now_iso(),
        summary: instruction.chars().take(120).collect(),
    });
    let _ = state_file::save(&config.data_dir(), &gstate);

    let spawn = vox_core::adapters::worker::WorkerSpawn {
        limits: config.spawn_limits(),
        claude_bin: config.claude_bin_resolved(),
        cwd: planned.workspace_root.clone(),
        session_id: planned.session.session_id.clone(),
        instruction: instruction.clone(),
        directives: vox_core::domain::directives::parse(&instruction),
    };
    let result = vox_core::adapters::worker::run(
        &spawn,
        &mut |_running| {},
        &mut |tool, input| {
            // Surface in the UI and block this worker thread on the click.
            let request_id = format!("{task_id}-{}", Utc::now().format("%H%M%S%f"));
            let (tx, rx) = mpsc::channel();
            pending.0.lock().unwrap().insert(request_id.clone(), tx);
            let _ = app.emit(
                "vox-permission",
                serde_json::json!({
                    "request_id": request_id,
                    "task_id": task_id,
                    "tool_name": tool,
                    "input": input,
                }),
            );
            rx.recv_timeout(std::time::Duration::from_secs(300))
                .unwrap_or(PermissionDecision::Deny)
        },
        &mut |event| {
            if let ClaudeEvent::ToolUse { name, input } = event {
                emit_event(
                    &app,
                    serde_json::json!({ "kind": "worker", "task_id": task_id, "name": name, "input": input }),
                );
            }
        },
    );

    // Ledger before anything else: even failed turns burned tokens.
    if let Ok(turn) = &result {
        let workspace_str = planned.workspace_root.display().to_string();
        record_live_spend(
            &config,
            vox_core::domain::spend::SpendKind::Dispatch,
            &vox_core::domain::spend::SpendMeta {
                task_id: Some(&task_id),
                session_id: Some(&planned.session.session_id),
                workspace: Some(&workspace_str),
                ..Default::default()
            },
            turn,
        );
    }

    let (status, out) = match &result {
        Ok(turn) if !turn.is_error => (
            WorkerStatus::Done,
            DispatchOut::Done {
                task_id: task_id.clone(),
                summary: turn.raw.clone(),
                cost_usd: turn.cost_usd,
            },
        ),
        Ok(turn) => (
            WorkerStatus::Failed,
            DispatchOut::Failed {
                task_id: task_id.clone(),
                summary: turn.raw.clone(),
                cost_usd: turn.cost_usd,
            },
        ),
        Err(err) => (
            WorkerStatus::Failed,
            DispatchOut::Failed {
                task_id: task_id.clone(),
                summary: format!("{err:#}"),
                cost_usd: None,
            },
        ),
    };

    let mut gstate = state_file::load(&config.data_dir());
    if let Some(record) = gstate.workers.iter_mut().find(|w| w.task_id == task_id) {
        record.status = status;
        if let Ok(turn) = &result {
            record.summary = turn.raw.chars().take(200).collect();
        }
    }
    let _ = state_file::save(&config.data_dir(), &gstate);
    let _ = memory_files::append_state(
        &planned.workspace_root,
        &format!("- {} {task_id} [{:?}] {instruction}", now_iso(), status),
    );
    Ok(out)
}

#[derive(Serialize)]
struct SpendAggOut {
    key: String,
    cost_usd: f64,
    input: u64,
    output: u64,
    cache_read: u64,
    cache_created: u64,
    turns: u64,
    errors: u64,
}

impl From<vox_core::ports::SpendAgg> for SpendAggOut {
    fn from(a: vox_core::ports::SpendAgg) -> Self {
        Self {
            key: a.key,
            cost_usd: a.cost_usd,
            input: a.usage.input,
            output: a.usage.output,
            cache_read: a.usage.cache_read,
            cache_created: a.usage.cache_created,
            turns: a.turns,
            errors: a.errors,
        }
    }
}

fn spend_group(group: &str) -> vox_core::ports::SpendGroup {
    use vox_core::ports::SpendGroup;
    match group {
        "model" => SpendGroup::Model,
        "label" => SpendGroup::Label,
        "workspace" => SpendGroup::Workspace,
        "day" => SpendGroup::Day,
        "session" => SpendGroup::Session,
        _ => SpendGroup::Kind,
    }
}

/// Ledger aggregation for the UI. `source` keeps USD (live) and token
/// history (jsonl) apart — the invariant of the whole ledger.
#[tauri::command(async)]
fn spend_summary(
    since: Option<String>,
    group: String,
    source: String,
) -> Result<Vec<SpendAggOut>, String> {
    use vox_core::ports::{SpendLedger, SpendQuery};
    let config = Config::load();
    let store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let query = SpendQuery {
        since,
        group: spend_group(&group),
        source: if source == "jsonl" {
            vox_core::domain::spend::SpendSource::Jsonl
        } else {
            vox_core::domain::spend::SpendSource::Live
        },
    };
    Ok(store
        .spend_summary(&query)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(SpendAggOut::from)
        .collect())
}

#[tauri::command(async)]
fn spend_top_sessions(since: String, limit: usize) -> Result<Vec<SpendAggOut>, String> {
    use vox_core::ports::{SessionStore, SpendLedger};
    let config = Config::load();
    let store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let mut aggs: Vec<SpendAggOut> = store
        .spend_top_sessions(&since, limit)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(SpendAggOut::from)
        .collect();
    // Session ids mean nothing to a human: swap in the session title.
    for agg in &mut aggs {
        if let Ok(Some(path)) = store.session_path(&agg.key) {
            if let Ok(Some((summary, _, _))) = store.file_state(&path) {
                if let Some(title) = summary.title.or(summary.last_prompt) {
                    agg.key = title.chars().take(48).collect();
                }
            }
        }
    }
    Ok(aggs)
}

#[derive(Serialize)]
struct ContextWeight {
    /// input + cache_read + cache_created of the session's last live turn.
    last_total_tokens: u64,
    context_window: Option<u64>,
    pct: Option<f64>,
}

/// How full a session's context window is, from its last live turn.
#[tauri::command(async)]
fn session_context_weight(session_id: String) -> Result<ContextWeight, String> {
    let config = Config::load();
    let store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let weight = store
        .last_context_weight(&session_id)
        .map_err(|e| e.to_string())?;
    let (tokens, window) = weight.unwrap_or((0, None));
    Ok(ContextWeight {
        last_total_tokens: tokens,
        context_window: window,
        pct: window
            .filter(|w| *w > 0)
            .map(|w| (tokens as f64 / w as f64).min(1.0)),
    })
}

#[derive(Serialize)]
struct SessionStats {
    title: Option<String>,
    /// Session log size on disk: the weight a resume drags along.
    size_mb: f64,
    /// Lines in the log (rough message/event count).
    entries: usize,
    last_ts: Option<String>,
}

/// Cheap local stats for one session (drives the scope popover). The size
/// is the same signal the gate uses to warn about expensive resumes.
#[tauri::command(async)]
fn session_stats(session_id: String) -> Result<SessionStats, String> {
    use vox_core::ports::SessionStore;
    let config = Config::load();
    let store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let path = store
        .session_path(&session_id)
        .map_err(|e| e.to_string())?
        .ok_or("sessão não está no índice")?;
    let (title, last_ts) = store
        .file_state(&path)
        .map_err(|e| e.to_string())?
        .map(|(s, _, _)| (s.title, s.last_ts))
        .unwrap_or_default();
    let size_mb = std::fs::metadata(&path)
        .map(|m| m.len() as f64 / 1_048_576.0)
        .unwrap_or(0.0);
    let entries = std::fs::read_to_string(&path)
        .map(|c| c.lines().count())
        .unwrap_or(0);
    Ok(SessionStats {
        title,
        size_mb,
        entries,
        last_ts,
    })
}

#[derive(Serialize)]
struct TranscriptOut {
    /// The session's own title, as shown in Claude Code.
    session_title: Option<String>,
    entries: Vec<vox_core::domain::transcript::Entry>,
}

/// Read-only history of a past session, straight from its log file.
/// Costs nothing: no process spawned, no tokens.
#[tauri::command(async)]
fn read_transcript(session_id: String, limit: Option<usize>) -> Result<TranscriptOut, String> {
    use vox_core::ports::SessionStore;
    let config = Config::load();
    let store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let path = store
        .session_path(&session_id)
        .map_err(|e| e.to_string())?
        .ok_or("sessão não está no índice (rode: vox index)")?;
    let (session_title, _, _) = store
        .file_state(&path)
        .map_err(|e| e.to_string())?
        .map(|(s, o, m)| (s.title, o, m))
        .unwrap_or_default();
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(TranscriptOut {
        session_title,
        entries: vox_core::domain::transcript::tail_entries(content.lines(), limit.unwrap_or(200)),
    })
}

/// Best session for a spoken/typed query, via the same topic search the
/// snapshot uses. Local and free.
#[tauri::command(async)]
fn find_session(query: String) -> Result<Option<serde_json::Value>, String> {
    use vox_core::ports::SessionStore;
    let config = Config::load();
    let store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let terms = vox_core::domain::dispatch::significant_terms(&query);
    let hits = store.search_sessions(&terms, 1).map_err(|e| e.to_string())?;
    Ok(hits.first().map(|s| {
        serde_json::json!({ "session_id": s.session_id, "title": s.title })
    }))
}

#[tauri::command]
fn board_move(title: String, status: vox_core::domain::board::TaskStatus) -> Result<(), String> {
    use vox_core::ports::SessionStore;
    let config = Config::load();
    let mut store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let tasks = store.board().map_err(|e| e.to_string())?;
    let tasks = vox_core::domain::board::set_status(tasks, &title, status, &now_iso());
    store.save_board(&tasks).map_err(|e| e.to_string())
}

fn with_board<T>(
    f: impl FnOnce(&mut SqliteStore, Vec<vox_core::domain::board::Task>) -> anyhow::Result<T>,
) -> Result<T, String> {
    use vox_core::ports::SessionStore;
    let config = Config::load();
    let mut store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let tasks = store.board().map_err(|e| e.to_string())?;
    f(&mut store, tasks).map_err(|e| e.to_string())
}

#[tauri::command]
fn board_rename(title: String, new_title: String) -> Result<(), String> {
    with_board(|store, tasks| {
        use vox_core::ports::SessionStore;
        let tasks = vox_core::domain::board::rename(tasks, &title, &new_title, &now_iso());
        store.save_board(&tasks)
    })
}

#[tauri::command]
fn board_pin(title: String) -> Result<(), String> {
    with_board(|store, tasks| {
        use vox_core::ports::SessionStore;
        let tasks = vox_core::domain::board::toggle_pin(tasks, &title);
        store.save_board(&tasks)
    })
}

#[derive(Serialize)]
struct GateOut {
    acao: vox_core::domain::gate::GateAction,
    confianca: f64,
    motivo: String,
    aviso: Option<String>,
    task_alvo: Option<String>,
    needs_confirmation: bool,
    cost_usd: Option<f64>,
}

/// The pre-execution evaluator: one cheap haiku call that decides where a
/// message goes BEFORE anything expensive runs. Runs with zero tools.
#[tauri::command(async)]
fn evaluate(
    message: String,
    focused_task: Option<String>,
    focused_session: Option<String>,
) -> Result<GateOut, String> {
    use std::io::{BufRead, BufReader, Write};
    use vox_core::domain::gate;
    use vox_core::ports::SessionStore;

    let config = Config::load();
    let store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;

    // Real session title + size hint: mismatches and heavy sessions are
    // exactly what the evaluator must warn about.
    let mut session_title = None;
    let mut size_hint = String::new();
    if let Some(sid) = &focused_session {
        if let Ok(Some(path)) = store.session_path(sid) {
            if let Ok(Some((summary, _, _))) = store.file_state(&path) {
                session_title = summary.title;
            }
            if let Ok(meta) = std::fs::metadata(&path) {
                let mb = meta.len() as f64 / 1_048_576.0;
                if mb > 2.0 {
                    size_hint = format!(" [historico de {mb:.1}MB, turno pode ser caro]");
                }
            }
        }
    }
    let board_lines: Vec<String> = store
        .board()
        .unwrap_or_default()
        .iter()
        .map(|t| format!("- {} [{:?}]", t.title, t.status))
        .collect();

    let ledger_session = focused_session.clone();
    let ctx = gate::GateContext {
        focused_task,
        focused_session,
        focused_session_title: session_title.map(|t| format!("{t}{size_hint}")),
        board_lines,
        live_workers: state_file::load(&config.data_dir())
            .workers
            .iter()
            .filter(|w| matches!(w.status, WorkerStatus::Running))
            .map(|w| w.summary.clone())
            .collect(),
    };

    let mut child = std::process::Command::new(config.claude_bin_resolved())
        .current_dir(config.data_dir())
        .args([
            "-p",
            "--input-format",
            "stream-json",
            "--model",
            "haiku",
            "--effort",
            "low",
            "--output-format",
            "stream-json",
            "--verbose",
            "--json-schema",
            gate::GATE_SCHEMA,
            "--tools",
            "",
            "--setting-sources",
            "",
            "--system-prompt",
            gate::GATE_SYSTEM_PROMPT,
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;

    let mut stdin = child.stdin.take().expect("piped stdin");
    let prompt = gate::build_prompt(&message, &ctx);
    stdin
        .write_all(
            vox_core::domain::claude_event::user_message(&prompt, None).as_bytes(),
        )
        .and_then(|_| stdin.write_all(b"\n"))
        .map_err(|e| e.to_string())?;
    drop(stdin);

    let stdout = child.stdout.take().expect("piped stdout");
    let result = BufReader::new(stdout)
        .lines()
        .map_while(Result::ok)
        .find_map(|line| match vox_core::domain::claude_event::parse(&line) {
            ClaudeEvent::Result(r) => Some(r),
            _ => None,
        });
    let _ = child.wait();

    let turn = result.ok_or("avaliador não respondeu")?;
    let decision_parse = serde_json::from_str::<vox_core::domain::gate::GateDecision>(&turn.raw);
    // Ledger with the verdict as outcome (feeds the savings counters);
    // a failed parse still cost a haiku turn.
    let outcome = match &decision_parse {
        Ok(d) => format!(
            "gate:{}",
            serde_json::to_string(&d.acao).unwrap_or_default().trim_matches('"')
        ),
        Err(_) => "gate:parse_error".into(),
    };
    record_live_spend(
        &config,
        vox_core::domain::spend::SpendKind::Gate,
        &vox_core::domain::spend::SpendMeta {
            label: Some("avaliador"),
            session_id: ledger_session.as_deref(),
            outcome: Some(&outcome),
            ..Default::default()
        },
        &turn,
    );
    let decision = decision_parse
        .map_err(|e| format!("gate parse: {e}"))?
        .sanitized();
    Ok(GateOut {
        needs_confirmation: decision.needs_confirmation(),
        acao: decision.acao,
        confianca: decision.confianca,
        motivo: decision.motivo,
        aviso: decision.aviso,
        task_alvo: decision.task_alvo,
        cost_usd: turn.cost_usd,
    })
}

/// Local board command spoken by the user ("mostra o log da X"). Resolves the
/// task by term overlap and performs the action; no LLM, no tokens.
#[tauri::command]
fn task_command(text: String) -> Result<Option<serde_json::Value>, String> {
    use vox_core::domain::task_command::TaskCommand;
    let Some(command) = vox_core::domain::task_command::parse(&text) else {
        // Fallback: "abre <nome>" without the word "projeto" ("ok, então
        // abra workspace fábrica"). Only fires when a REGISTERED project name
        // appears in the sentence — never guesses.
        let lower = text.to_lowercase();
        if ["abre", "abra", "abrir"].iter().any(|v| lower.contains(v)) {
            let config = Config::load();
            let projects = load_projects(&config);
            if let Some(hit) = vox_core::domain::project::find_spoken(&projects, &text) {
                return Ok(Some(serde_json::json!({
                    "kind": "open_project", "title": hit.name, "path": hit.path,
                    "instruction": null,
                })));
            }
        }
        return Ok(None);
    };
    // Project/file commands resolve without the board.
    match &command {
        TaskCommand::OpenFile { query, project } => {
            return Ok(Some(serde_json::json!({
                "kind": "open_file", "query": query, "project": project,
            })));
        }
        TaskCommand::AddProject { path } => {
            return Ok(Some(match project_add(path.clone()) {
                Ok(entry) => serde_json::json!({
                    "kind": "project_added", "title": entry.name, "path": entry.path,
                }),
                Err(message) => serde_json::json!({
                    "kind": "project_error", "title": message,
                }),
            }));
        }
        TaskCommand::NewChat { project, instruction } => {
            let config = Config::load();
            let projects = load_projects(&config);
            return Ok(Some(
                match vox_core::domain::project::find(&projects, project) {
                    Some(hit) => serde_json::json!({
                        "kind": "new_chat", "title": hit.name, "path": hit.path,
                        "instruction": instruction,
                    }),
                    None => serde_json::json!({ "kind": "not_found", "query": project }),
                },
            ));
        }
        TaskCommand::OpenProject { query, instruction } => {
            let config = Config::load();
            let projects = load_projects(&config);
            let hit = vox_core::domain::project::find(&projects, query)
                .or_else(|| vox_core::domain::project::find_spoken(&projects, query));
            return Ok(Some(match hit {
                Some(hit) => serde_json::json!({
                    "kind": "open_project", "title": hit.name, "path": hit.path,
                    "instruction": instruction,
                }),
                None => serde_json::json!({ "kind": "not_found", "query": query }),
            }));
        }
        TaskCommand::OpenHq { tab } => {
            return Ok(Some(serde_json::json!({ "kind": "open_hq", "tab": tab })));
        }
        _ => {}
    }
    let query = match &command {
        TaskCommand::Open(q) | TaskCommand::Pin(q) | TaskCommand::Archive(q) => q,
        TaskCommand::Switch { query, .. } => query,
        TaskCommand::Rename { query, .. } => query,
        TaskCommand::OpenFile { .. }
        | TaskCommand::AddProject { .. }
        | TaskCommand::NewChat { .. }
        | TaskCommand::OpenProject { .. }
        | TaskCommand::OpenHq { .. } => {
            unreachable!("handled above")
        }
    };
    with_board(|store, tasks| {
        use vox_core::ports::SessionStore;
        let Some(task) = vox_core::domain::board::find(&tasks, query) else {
            return Ok(Some(serde_json::json!({ "kind": "not_found", "query": query })));
        };
        let title = task.title.clone();
        match &command {
            TaskCommand::Open(_) => Ok(Some(serde_json::json!({
                "kind": "open", "title": title,
                "session_id": task.session_ids.last(),
            }))),
            TaskCommand::Switch { instruction, .. } => Ok(Some(serde_json::json!({
                "kind": "switch", "title": title,
                "session_id": task.session_ids.last(),
                "note": task.note,
                "instruction": instruction,
            }))),
            TaskCommand::Rename { title: new, .. } => {
                let tasks = vox_core::domain::board::rename(tasks, &title, new, &now_iso());
                store.save_board(&tasks)?;
                Ok(Some(serde_json::json!({ "kind": "renamed", "title": new })))
            }
            TaskCommand::Pin(_) => {
                let tasks = vox_core::domain::board::toggle_pin(tasks, &title);
                store.save_board(&tasks)?;
                Ok(Some(serde_json::json!({ "kind": "pinned", "title": title })))
            }
            TaskCommand::Archive(_) => {
                let tasks = vox_core::domain::board::archive(tasks, &title);
                store.save_board(&tasks)?;
                Ok(Some(serde_json::json!({ "kind": "archived", "title": title })))
            }
            TaskCommand::OpenFile { .. }
            | TaskCommand::AddProject { .. }
            | TaskCommand::NewChat { .. }
            | TaskCommand::OpenProject { .. }
            | TaskCommand::OpenHq { .. } => unreachable!("handled above"),
        }
    })
}

#[tauri::command]
fn board_archive(title: String) -> Result<(), String> {
    use vox_core::ports::SessionStore;
    let config = Config::load();
    let mut store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let tasks = store.board().map_err(|e| e.to_string())?;
    let tasks = vox_core::domain::board::archive(tasks, &title);
    store.save_board(&tasks).map_err(|e| e.to_string())
}

/// Open (or focus) the dedicated window of one project — the VSCode-style
/// "one project, one window" model. The main window stays the orchestrator.
#[tauri::command]
fn open_project_window(app: AppHandle, name: String, path: String) -> Result<(), String> {
    let label: String = format!(
        "proj-{}",
        path.chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>()
    )
    .chars()
    .take(60)
    .collect();
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.set_focus();
        return Ok(());
    }
    let url = format!(
        "index.html?project={}&name={}",
        urlencoding::encode(&path),
        urlencoding::encode(&name)
    );
    tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::App(url.into()))
        .title(format!("Vox — {name}"))
        .inner_size(1280.0, 820.0)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Bring the mother window to the front, optionally switching its tab
/// (board/custos live THERE — global views; project windows only filter).
#[tauri::command]
fn focus_main(app: AppHandle, tab: Option<String>) -> Result<(), String> {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }
    if let Some(tab) = tab {
        let _ = app.emit_to("main", "vox", serde_json::json!({ "kind": "main_tab", "tab": tab }));
    }
    Ok(())
}

/// Answer a pending permission request (live worker first, one-shot channel
/// as fallback).
#[tauri::command]
fn approve(
    pending: State<'_, Pending>,
    perms: State<'_, WorkerPermissions>,
    live: State<'_, LiveWorkers>,
    request_id: String,
    allow: bool,
) {
    let decision = if allow {
        PermissionDecision::Allow
    } else {
        PermissionDecision::Deny
    };
    if let Some(task_id) = perms.0.lock().unwrap().remove(&request_id) {
        if let Some(worker) = live.0.lock().unwrap().get(&task_id) {
            let _ = worker.worker.respond_permission(&request_id, decision);
        }
        return;
    }
    if let Some(tx) = pending.0.lock().unwrap().remove(&request_id) {
        let _ = tx.send(decision);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Pending(Mutex::new(HashMap::new())))
        .manage(LiveWorkers(Mutex::new(HashMap::new())))
        .manage(WorkerPermissions(Mutex::new(HashMap::new())))
        .setup(|app| {
            // Warm whisper in the background so the first mic use is instant.
            let _handle = app.handle().clone();
            std::thread::spawn(|| {
                let config = Config::load();
                let _ = stt(&config);
            });
            // Register the global mic hotkey (config `hotkey`).
            {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                let hotkey = Config::load().hotkey;
                if let Err(err) = app.handle().global_shortcut().register(hotkey.as_str()) {
                    eprintln!("vox: hotkey global '{hotkey}' não registrada: {err}");
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            overview,
            use_context,
            route_text,
            speak,
            speak_stop,
            hear_once,
            hear_stop,
            ask_text,
            dispatch_text,
            worker_start,
            worker_send,
            worker_stop,
            chat_start,
            project_add,
            project_remove,
            project_files,
            file_read,
            file_save,
            read_transcript,
            find_session,
            session_stats,
            spend_summary,
            spend_top_sessions,
            session_context_weight,
            task_command,
            evaluate,
            board_move,
            board_rename,
            board_pin,
            board_archive,
            open_project_window,
            focus_main,
            approve
        ])
        // Global hotkey (default cmd+shift+space, config `hotkey`): from
        // ANY app, focus the mother and open the mic — the JARVIS button.
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        if let Some(main) = app.get_webview_window("main") {
                            let _ = main.show();
                            let _ = main.set_focus();
                        }
                        let _ = app.emit_to("main", "vox", serde_json::json!({ "kind": "hotkey_mic" }));
                    }
                })
                .build(),
        )
        // The mother window commands everything: closing it closes the app
        // (project windows included). Closing a project window is local.
        .on_window_event(|window, event| {
            if window.label() == "main"
                && matches!(event, tauri::WindowEvent::CloseRequested { .. })
            {
                window.app_handle().exit(0);
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running vox");
}

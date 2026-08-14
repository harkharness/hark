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
) -> AskDeps<'a> {
    let state = state_file::load(&config.data_dir());
    AskDeps {
        active_context: state.active_context,
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
        _p: &str,
        _i: Option<(&str, &str)>,
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
}

#[tauri::command]
fn overview() -> Overview {
    let config = Config::load();
    let state = state_file::load(&config.data_dir());
    let mut workers = state.workers;
    workers.reverse(); // newest first
    workers.truncate(12);
    Overview {
        contexts: config.context_names(),
        active: state
            .active_context
            .unwrap_or_else(|| config.default_context.clone()),
        workers,
    }
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
fn speak(text: String) {
    let config = Config::load();
    std::thread::spawn(move || {
        let _ = SayTts {
            voice: config.voice,
        }
        .speak(&text);
    });
}

#[tauri::command(async)]
fn hear_once() -> Result<String, String> {
    let config = Config::load();
    let stt = stt(&config).ok_or("whisper model missing (run: vox setup)")?;
    let tts = SayTts {
        voice: config.voice.clone(),
    };
    tts.beep(vox_core::ports::Cue::Listening);
    let audio = CpalMic::default()
        .record_utterance()
        .map_err(|e| e.to_string())?;
    tts.beep(vox_core::ports::Cue::Captured);
    stt.transcribe(&audio).map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct ReplyOut {
    fala: String,
    detalhes: String,
    itens: Vec<String>,
    cost_usd: Option<f64>,
}

#[tauri::command(async)]
fn ask_text(
    app: AppHandle,
    question: String,
    image_b64: Option<String>,
    media_type: Option<String>,
) -> Result<ReplyOut, String> {
    let config = Config::load();
    let mut store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let live = ClaudeAgentsCli {
        claude_bin: config.claude_bin.clone(),
    };
    let runner = ClaudeCli {
        claude_bin: config.claude_bin.clone(),
        model: config.model.clone(),
        work_dir: config.data_dir(),
    };
    let mut deps = build_deps(&config, &mut store, &live, &runner);
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
        Some(VoiceReply { fala, detalhes, itens }) => Ok(ReplyOut {
            fala,
            detalhes,
            itens,
            cost_usd: result.cost_usd,
        }),
        None if !result.is_error => Ok(ReplyOut {
            fala: result.raw.clone(),
            detalhes: result.raw,
            itens: vec![],
            cost_usd: result.cost_usd,
        }),
        None => Err(result.raw),
    }
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum DispatchOut {
    Done {
        task_id: String,
        summary: String,
        cost_usd: Option<f64>,
    },
    Failed {
        task_id: String,
        summary: String,
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
            claude_bin: config.claude_bin.clone(),
        };
        let mut deps = build_deps(&config, &mut store, &live, &NoopRunner);
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
        claude_bin: config.claude_bin.clone(),
        cwd: planned.workspace_root.clone(),
        session_id: planned.session.session_id.clone(),
        instruction: instruction.clone(),
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
            },
        ),
        Err(err) => (
            WorkerStatus::Failed,
            DispatchOut::Failed {
                task_id: task_id.clone(),
                summary: format!("{err:#}"),
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

#[tauri::command]
fn approve(pending: State<'_, Pending>, request_id: String, allow: bool) {
    if let Some(tx) = pending.0.lock().unwrap().remove(&request_id) {
        let _ = tx.send(if allow {
            PermissionDecision::Allow
        } else {
            PermissionDecision::Deny
        });
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Pending(Mutex::new(HashMap::new())))
        .setup(|app| {
            // Warm whisper in the background so the first mic use is instant.
            let _handle = app.handle().clone();
            std::thread::spawn(|| {
                let config = Config::load();
                let _ = stt(&config);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            overview,
            use_context,
            route_text,
            speak,
            hear_once,
            ask_text,
            dispatch_text,
            approve
        ])
        .run(tauri::generate_context!())
        .expect("error while running vox");
}

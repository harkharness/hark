//! Tauri driver: the desktop window over the same hark-core used by the CLI.

mod terminal;
mod voice;

use serde::Serialize;
use std::collections::HashMap;
use std::sync::{mpsc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager, State};
use hark_plugin_claude::cli::ClaudeCli;
use hark_core::adapters::git_collect::GitCli;
use hark_plugin_claude::live::ClaudeAgentsCli;
use hark_core::adapters::memory_files::{self, HarkDir};
use hark_core::adapters::sqlite_store::SqliteStore;
use hark_core::adapters::state_file;
use hark_core::adapters::{cpal_audio::CpalMic, say_tts::SayTts, whisper_stt::WhisperStt};
use hark_core::app::ask::{ask_with_image, AskDeps};
use hark_core::app::dispatch::{plan, Plan};
use hark_core::chrono::{SecondsFormat, Utc};
use hark_core::config::Config;
use hark_core::domain::reply::VoiceReply;
use hark_agent::{AgentEvent as ClaudeEvent, PermissionDecision};
use hark_core::domain::memory::{WorkerRecord, WorkerStatus};
use hark_core::ports::{AgentBackend, AgentRunner, AgentSession, AudioIn, SessionSpec, Tts};
use hark_plugin_claude::backend::ClaudeBackend;

/// Permission requests waiting for a click, keyed by request_id.
struct Pending(Mutex<HashMap<String, mpsc::Sender<PermissionDecision>>>);

/// Every undecided permission ask, newest LAST: (request_id, label, tool).
/// The HUD's step-0 check answers the newest one by voice from anywhere.
#[derive(Default)]
pub(crate) struct PermLog(pub(crate) Mutex<Vec<(String, String, String)>>);

/// The active backend for one agent id. v1: always the native claude
/// plugin — F9.1 turns this into the registry lookup.
fn backend_for(config: &Config) -> std::sync::Arc<dyn AgentBackend> {
    std::sync::Arc::new(ClaudeBackend {
        bin: config.claude_bin_resolved(),
        work_dir: config.data_dir(),
    })
}

/// Restart identity: each spawned session gets a generation number; a
/// reader thread only reports the exit of ITS generation (a pid can be
/// reused, and not every backend has one).
static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// A live session plus everything needed to restart it on the same task.
struct WorkerHandle {
    session: std::sync::Arc<dyn AgentSession>,
    spec: SessionSpec,
    generation: u64,
}

/// Conversational workers still alive, keyed by task_id.
struct LiveWorkers(Mutex<HashMap<String, std::sync::Arc<WorkerHandle>>>);

/// Permission requests raised by live workers: request_id -> task_id.
struct WorkerPermissions(Mutex<HashMap<String, String>>);

/// Whisper loads once per process (heavy); everything else is per-call.
/// Only a LOADED model is cached. Caching the failure was the wizard bug:
/// the first call happens on a machine with no model yet, and a
/// OnceLock<Result<..>> then answered "missing" for the rest of the
/// process — including the mic step right after the download finished.
static STT: OnceLock<WhisperStt> = OnceLock::new();
/// A load is in flight (or finished). Not a mutex on purpose: the UI path
/// must never WAIT behind a 466MB read. It waited, once — on an Intel Mac
/// the boot warmup was still going when the user pressed the mic, and
/// hear_once blocked on the lock: no beep, no capture, and Esc inert
/// because the stop flag is only read inside the capture loop.
static STT_LOADING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// The model file itself is absent (nothing to load until it downloads).
static STT_ABSENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whisper's initial prompt, built from what THIS machine talks about:
/// registered project names and the titles of recent sessions, on top of
/// the configured vocabulary. A generic list is why "Gladius Hydrator"
/// came back as "o gladus de dreater".
fn speech_bias(config: &Config) -> String {
    let data_dir = config.data_dir();
    let state = hark_core::adapters::state_file::load(&data_dir);
    let projects: Vec<String> = state.projects.iter().map(|p| p.name.clone()).collect();

    // Session titles are a nice-to-have: a missing index must never stop
    // the microphone from loading.
    use hark_core::ports::SessionStore;
    let titles: Vec<String> = SqliteStore::open(&data_dir.join("index.db"))
        .ok()
        .and_then(|store| {
            let since = (Utc::now() - hark_core::chrono::Duration::days(30)).to_rfc3339();
            store.sessions_since(&since).ok()
        })
        .map(|sessions| sessions.iter().filter_map(|s| s.title.clone()).collect())
        .unwrap_or_default();

    // Whisper truncates its prompt around 224 tokens; stay well under.
    hark_core::domain::vocab::speech_bias(&config.vocab, &projects, &titles, 600)
}

/// What the microphone can count on right now.
enum Speech {
    Ready(&'static WhisperStt),
    /// Being read into memory; the caller should say so, not block.
    Loading,
    /// No model on disk yet.
    Absent,
}

/// Start reading the model, at most once at a time. Returns immediately.
fn ensure_speech_loading() {
    use std::sync::atomic::Ordering::SeqCst;
    if STT.get().is_some() || STT_LOADING.swap(true, SeqCst) {
        return;
    }
    STT_ABSENT.store(false, SeqCst);
    std::thread::spawn(|| {
        let config = Config::load();
        match WhisperStt::load(
            &config.whisper_model_path(),
            &config.stt_language(),
            &[speech_bias(&config)],
        ) {
            Ok(loaded) => {
                loaded.warmup();
                let _ = STT.set(loaded);
            }
            Err(err) => {
                eprintln!("hark: modelo de voz nao carregou: {err}");
                STT_ABSENT.store(true, SeqCst);
                // Let a later attempt retry — the file may be downloading.
                STT_LOADING.store(false, SeqCst);
            }
        }
    });
}

/// Never blocks and never loads inline: the mic path asks and reacts.
fn speech() -> Speech {
    use std::sync::atomic::Ordering::SeqCst;
    if let Some(loaded) = STT.get() {
        return Speech::Ready(loaded);
    }
    ensure_speech_loading();
    match STT_ABSENT.load(SeqCst) {
        true => Speech::Absent,
        false => Speech::Loading,
    }
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn emit_event(app: &AppHandle, payload: serde_json::Value) {
    let _ = app.emit("hark", payload);
}

fn build_deps<'a>(
    config: &'a Config,
    store: &'a mut SqliteStore,
    live: &'a ClaudeAgentsCli,
    runner: &'a dyn AgentRunner,
    active_project: Option<hark_core::domain::project::Project>,
) -> AskDeps<'a> {
    let state = state_file::load(&config.data_dir());
    AskDeps {
        active_context: state.active_context,
        projects: state.projects,
        active_project,
        workers: state.workers,
        journal: &HarkDir,
        store,
        live,
        repos: &GitCli,
        runner,
        config,
        indexer: &hark_plugin_claude::history::ClaudeHistory,
    }
}

struct NoopRunner;
impl AgentRunner for NoopRunner {
    fn ask(
        &self,
        _request: &hark_core::ports::TurnRequest,
        _e: &mut dyn FnMut(&ClaudeEvent),
    ) -> anyhow::Result<hark_plugin_claude::stream::TurnResult> {
        anyhow::bail!("not used")
    }
}

#[derive(Serialize)]
struct Overview {
    contexts: Vec<String>,
    active: String,
    workers: Vec<WorkerRecord>,
    board: Vec<hark_core::domain::board::Task>,
    projects: Vec<hark_core::domain::project::Project>,
    theme: String,
    /// Config default permission mode for new workers ("" = CLI default).
    default_mode: String,
    /// UI language ("pt" | "en") — separate from the spoken one.
    ui_language: String,
    /// SPOKEN language (STT/TTS) — drives the speech dictionary.
    language: String,
    /// The assistant's own name (chat header, feed, announcements).
    assistant_name: String,
    /// The human's first name (from $USER), for the greeting.
    user_name: String,
}

/// The editable project list. First run seeds it from what the machine
/// already knows: board workspaces and configured context repos.
fn load_projects(config: &Config) -> Vec<hark_core::domain::project::Project> {
    use hark_core::domain::project;
    use hark_core::ports::SessionStore;
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
    use hark_core::ports::SessionStore;
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
        ui_language: config.ui_language,
        language: config.language,
        assistant_name: config.assistant_name,
        user_name: std::env::var("USER")
            .ok()
            .and_then(|u| {
                let first = u.split(['.', '_', '-']).next()?.to_string();
                let mut chars = first.chars();
                chars
                    .next()
                    .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
            })
            .unwrap_or_default(),
        default_mode: config.worker_mode,
    }
}

#[tauri::command]
fn project_add(path: String) -> Result<hark_core::domain::project::Project, String> {
    use hark_core::domain::project;
    let expanded = hark_core::config::expand_home(&path);
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
    state.projects = hark_core::domain::project::remove(state.projects, &key);
    state_file::save(&config.data_dir(), &state).map_err(|e| e.to_string())
}

/// Fuzzy file search inside a project (drives @mention and quick-open).
#[tauri::command(async)]
fn project_files(path: String, query: String, limit: Option<usize>) -> Vec<String> {
    let root = std::path::PathBuf::from(hark_core::config::expand_home(&path));
    let files = hark_core::adapters::fs_files::list_files(&root, 8000);
    hark_core::domain::file_search::search(
        &query,
        files.iter().map(|s| s.as_str()),
        limit.unwrap_or(30),
    )
}

/// Only files inside registered projects — plus ~/.claude (plans, session
/// logs the user clicks open) — are readable/writable from the UI.
fn guard_project_path(path: &str) -> Result<std::path::PathBuf, String> {
    let config = Config::load();
    let target = std::path::PathBuf::from(hark_core::config::expand_home(path))
        .canonicalize()
        .map_err(|e| format!("{path}: {e}"))?;
    let projects = load_projects(&config);
    let roots = projects
        .iter()
        .map(|proj| proj.path.clone())
        .chain(std::iter::once("~/.claude".to_string()));
    let allowed = roots.into_iter().any(|root| {
        std::path::PathBuf::from(hark_core::config::expand_home(&root))
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
    match hark_core::domain::intent::route(&text) {
        hark_core::domain::intent::Route::Dispatch => "dispatch".into(),
        hark_core::domain::intent::Route::Ask => "ask".into(),
    }
}

/// One mouth: utterances queue behind this lock instead of talking over
/// each other (two permission asks in a row used to speak simultaneously).
fn tts_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

/// Bumped by speak_stop: queued utterances from before the cut give up
/// instead of speaking stale news after Esc.
fn tts_generation() -> &'static std::sync::atomic::AtomicU64 {
    static GEN: std::sync::OnceLock<std::sync::atomic::AtomicU64> = std::sync::OnceLock::new();
    GEN.get_or_init(|| std::sync::atomic::AtomicU64::new(0))
}

#[tauri::command]
fn speak(app: AppHandle, text: String) {
    let config = Config::load();
    let generation = tts_generation().load(std::sync::atomic::Ordering::SeqCst);
    std::thread::spawn(move || {
        // Serialize: the second utterance WAITS for the first to finish.
        let _mouth = tts_lock().lock().unwrap_or_else(|e| e.into_inner());
        if tts_generation().load(std::sync::atomic::Ordering::SeqCst) != generation {
            return; // cut (Esc) while queued: stale news stays unsaid
        }
        // The voice orb follows these events (no audio analysis needed).
        emit_event(&app, serde_json::json!({ "kind": "speaking", "on": true }));
        let _ = SayTts {
            voice: config.voice,
        }
        .speak(&text);
        emit_event(&app, serde_json::json!({ "kind": "speaking", "on": false }));
    });
}

/// Cut any in-flight TTS immediately (Esc in the window) — and drop
/// whatever was queued behind it.
#[tauri::command]
fn speak_stop(app: AppHandle) {
    tts_generation().fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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

/// Kill-the-turn flag: `hear_abort` flips it and whatever `hear_once` is
/// doing — capturing OR transcribing — dies with `mic_aborted` instead of
/// delivering text. Separate from the stop flag on purpose: stop means
/// "I'm done talking, transcribe it"; abort means "forget the whole thing".
/// The distinction exists because on CPU (Intel) whisper grinds for tens
/// of seconds, and Esc used to be inert exactly there.
fn mic_abort_flag() -> std::sync::Arc<std::sync::atomic::AtomicBool> {
    static FLAG: std::sync::OnceLock<std::sync::Arc<std::sync::atomic::AtomicBool>> =
        std::sync::OnceLock::new();
    FLAG.get_or_init(|| std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)))
        .clone()
}

/// ONE microphone, one owner at a time. Concurrent hear_once calls used
/// to record in parallel over a single global stop flag; now the second
/// caller gets "mic_busy:<owner>" and decides (the HUD takes over with a
/// hear_stop + retry; modal loops just wait their turn).
#[derive(Default)]
struct MicLease(Mutex<Option<String>>);

struct MicGuard<'a>(&'a MicLease);
impl Drop for MicGuard<'_> {
    fn drop(&mut self) {
        *self.0 .0.lock().unwrap() = None;
    }
}

/// Narrates what the microphone is doing to every window. The windows own
/// a mic button and an orb but had no way to know: the phase used to be
/// implicit in a promise nobody could see, so the button stayed dead while
/// the machine listened. Dropping it says "idle" — so an error, an abort
/// or an early return can never leave a window stuck on "listening".
struct MicPhase {
    app: AppHandle,
    owner: String,
}

impl MicPhase {
    fn say(&self, phase: &str) {
        emit_event(
            &self.app,
            serde_json::json!({ "kind": "mic", "phase": phase, "owner": self.owner }),
        );
    }
}

impl Drop for MicPhase {
    fn drop(&mut self) {
        self.say("idle");
    }
}

#[tauri::command(async)]
fn hear_once(
    app: AppHandle,
    lease: State<'_, MicLease>,
    owner: Option<String>,
) -> Result<String, String> {
    let owner = owner.unwrap_or_else(|| "janela".into());
    {
        let mut current = lease.0.lock().unwrap();
        if let Some(holder) = current.as_deref() {
            return Err(format!("mic_busy:{holder}"));
        }
        *current = Some(owner.clone());
    }
    let _guard = MicGuard(&lease);
    let config = Config::load();
    // External dictation (FASE 8.3): STT is a commodity and the user may
    // prefer their own tool. Hark keeps the part nobody else has — the
    // targeting — and the HUD swaps the mic for a text field.
    if config.stt == "external" {
        return Err("mic_external".into());
    }
    // Clear the manual cut BEFORE anything slow, so an Esc pressed while
    // the model is still loading is not silently swallowed by a later
    // reset — and so the flag means "cut the capture I am about to start".
    let stop = mic_stop_flag();
    stop.store(false, std::sync::atomic::Ordering::SeqCst);
    let abort = mic_abort_flag();
    abort.store(false, std::sync::atomic::Ordering::SeqCst);
    let stt = match speech() {
        Speech::Ready(loaded) => loaded,
        // Codes, not prose: the window turns these into its own words.
        // Loading carries the size so the UI can explain the wait — on a
        // cold Intel Mac "loading" lasts long enough to read as broken.
        Speech::Loading => {
            let wanted = config.whisper_model_path();
            let size = wanted
                .file_name()
                .and_then(|f| f.to_str())
                .and_then(|name| {
                    hark_core::adapters::model_fetch::whisper_models()
                        .iter()
                        .find(|m| m.filename == name)
                })
                .map(|m| m.size_label);
            return Err(match size {
                Some(label) => format!("mic_loading:{label}"),
                None => "mic_loading".into(),
            });
        }
        Speech::Absent => return Err("mic_no_model".into()),
    };
    // Phase events let the UI say what the mic is DOING. The promise alone
    // can't: on CPU the transcription runs tens of seconds after the
    // capture ended, and "listening" the whole way read as frozen.
    let phase = MicPhase { app, owner };
    let tts = SayTts {
        voice: config.voice.clone(),
    };
    tts.beep(hark_core::ports::Cue::Listening);
    phase.say("capturing");
    let audio = CpalMic {
        stop,
        ..CpalMic::default()
    }
    .record_utterance()
    .map_err(|e| e.to_string())?;
    if abort.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("mic_aborted".into());
    }
    tts.beep(hark_core::ports::Cue::Captured);
    phase.say("transcribing");
    match stt.transcribe_with_abort(&audio, abort) {
        Ok(Some(text)) => Ok(text),
        Ok(None) => Err("mic_aborted".into()),
        Err(e) => Err(e.to_string()),
    }
}

/// Spoken verdict on whatever is pending (confirm modal, picker, warning
/// actions). Pure passthrough to the domain grammar — the ONE place that
/// decides what "sim", "a segunda" or a rephrase mean.
#[tauri::command]
fn interpret_verdict(
    utterance: String,
    options: Option<Vec<String>>,
    actions: Option<Vec<(String, Vec<String>)>>,
) -> Result<serde_json::Value, String> {
    use hark_core::domain::verdict::Verdict;
    let options = options.unwrap_or_default();
    let option_refs: Vec<&str> = options.iter().map(String::as_str).collect();
    let actions = actions.unwrap_or_default();
    let phrase_refs: Vec<Vec<&str>> = actions
        .iter()
        .map(|(_, ps)| ps.iter().map(String::as_str).collect())
        .collect();
    let action_refs: Vec<(&str, &[&str])> = actions
        .iter()
        .zip(&phrase_refs)
        .map(|((id, _), ps)| (id.as_str(), ps.as_slice()))
        .collect();
    Ok(
        match hark_core::domain::verdict::interpret(&utterance, &option_refs, &action_refs) {
            Verdict::Confirm { always } => serde_json::json!({ "kind": "confirm", "always": always }),
            Verdict::Deny => serde_json::json!({ "kind": "deny" }),
            Verdict::Pick(i) => serde_json::json!({ "kind": "pick", "index": i }),
            Verdict::Action(id) => serde_json::json!({ "kind": "action", "id": id }),
            Verdict::Instruction(text) => {
                serde_json::json!({ "kind": "instruction", "text": text })
            }
            Verdict::Unknown => serde_json::json!({ "kind": "unknown" }),
        },
    )
}

/// Esc while the orb is red: cut the capture NOW and transcribe what was
/// already said (the VAD can be slow in a noisy room).
#[tauri::command]
fn hear_stop() {
    mic_stop_flag().store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Esc once the words no longer matter: kill the in-flight `hear_once`
/// wherever it is. The stop flag rides along so a still-running capture
/// ends immediately instead of waiting out the VAD.
#[tauri::command]
fn hear_abort() {
    mic_abort_flag().store(true, std::sync::atomic::Ordering::SeqCst);
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
    images: Option<Vec<(String, String)>>,
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
        (Some(name), Some(path)) => Some(hark_core::domain::project::Project { name, path }),
        _ => None,
    };
    let mut deps = build_deps(&config, &mut store, &live, &runner, active_project);
    let images = images.unwrap_or_default();
    let result = ask_with_image(&question, &images, &mut deps, &mut |event| {
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

    match VoiceReply::from_turn(&result) {
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
        directives: hark_core::domain::directives::Directives,
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
    mode: Option<String>,
    model: Option<String>,
    fork: Option<bool>,
) -> Result<DispatchOut, String> {
    let fork = fork.unwrap_or(false);
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
        // A fork never co-writes: the owner's session only SEEDS the new
        // one, so "busy" does not apply (FASE 8.4 — parallel work while a
        // human holds the original at a terminal).
        Plan::TargetBusy(s) if fork => hark_core::app::dispatch::Planned {
            workspace_root: s.cwd.clone().unwrap_or_default().into(),
            session: s,
        },
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
        "# Hark dispatch {task_id}\n\n- session: {}\n- workspace: {}\n\n## Instruction\n\n{instruction}\n",
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

    // Mode precedence: spoken directive > window selector > config default.
    let mut directives = hark_core::domain::directives::parse(&instruction);
    directives.mode = directives
        .mode
        .or_else(|| mode.as_deref().and_then(hark_core::domain::directives::Mode::from_flag))
        .or_else(|| config.default_worker_mode());
    // Model precedence: spoken > window selector > router hint (FASE 8.6:
    // "investiga a causa raiz" routes UP by itself; never down).
    directives.model = directives.model.or(model.filter(|m| !m.is_empty()));
    if directives.model.is_none() {
        directives.model =
            hark_core::domain::intent::worker_model_hint(&instruction, &config.models());
    }
    let mut spawn = fresh_spawn(
        &config,
        planned.workspace_root.clone(),
        planned.session.session_id.clone(),
        instruction.clone(),
        directives.clone(),
    );
    spawn.fork = fork;
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
    spec: SessionSpec,
) -> anyhow::Result<()> {
    start_worker_titled(app, live, task_id, spec, None)
}

/// Same, with an explicit human title — the light restart opens a FRESH
/// session (empty session_id) whose instruction is a local brief; deriving
/// the title from that brief would fork the board card.
fn start_worker_titled(
    app: &AppHandle,
    live: &State<'_, LiveWorkers>,
    task_id: &str,
    spec: SessionSpec,
    title: Option<String>,
) -> anyhow::Result<()> {
    let backend = backend_for(&Config::load());
    let (session, rx) = backend.spawn(&spec)?;
    let generation = GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    live.0.lock().unwrap().insert(
        task_id.to_string(),
        std::sync::Arc::new(WorkerHandle { session, spec: spec.clone(), generation }),
    );

    // Reader loop: consume the backend's event stream, forward rich events
    // to the UI, route permission requests, keep going until it closes.
    let app2 = app.clone();
    let task2 = task_id.to_string();
    let workspace = spec.cwd.clone();
    let is_new_session = spec.session_id.is_empty();
    // The human name of this work: the session's title for resumes, the
    // instruction for brand-new sessions. Travels inside events so any
    // window (the mother above all) can speak about it by name.
    let board_title = if task_id == HARK_CHAT_TASK {
        // The chat speaks and asks permissions under its OWN name.
        Config::load().assistant_name
    } else {
        title.unwrap_or_else(|| {
            worker_board_title(
                (!spec.session_id.is_empty()).then_some(spec.session_id.as_str()),
                &spec.instruction,
            )
        })
    };
    let mut current_session = spec.session_id.clone();
    // Last phase reported to the window. The CLI streams its status many
    // times a second; only the transitions are worth an event.
    let mut last_phase: Option<hark_agent::AgentPhase> = None;
    // Which eco tools this spawn runs with — every turn's ledger row
    // carries it, so the costs panel can compare real per-tool averages.
    let eco_outcome = eco_fingerprint(&spec.envs);
    // The opening instruction IS a turn in flight (batching bookkeeping).
    app.state::<BatchState>()
        .0
        .lock()
        .unwrap()
        .entry(task_id.to_string())
        .or_default()
        .in_flight = true;
    std::thread::spawn(move || {
        let config = Config::load();
        for event in rx.iter() {
            match event {
                ClaudeEvent::SessionStarted { session_id, slash_commands } => {
                    current_session = session_id.clone();
                    // The init event names the session's slash commands:
                    // remember them per workspace for the "/" palette.
                    if !slash_commands.is_empty() {
                        let reg = app2.state::<SlashRegistry>();
                        let mut map = reg.0.lock().unwrap();
                        map.insert(workspace.display().to_string(), slash_commands.clone());
                        map.insert(String::new(), slash_commands);
                    }
                    if task2 == HARK_CHAT_TASK {
                        // The mother's chat lives OFF the board and OFF the
                        // worker registry; only its session id persists so
                        // the next send resumes the same conversation.
                        let mut gstate = state_file::load(&config.data_dir());
                        if gstate.hark_chat_session.as_deref() != Some(session_id.as_str()) {
                            gstate.hark_chat_session = Some(session_id.clone());
                            let _ = state_file::save(&config.data_dir(), &gstate);
                        }
                        emit_event(
                            &app2,
                            serde_json::json!({ "kind": "session_started",
                                "task_id": task2, "session_id": session_id }),
                        );
                        continue;
                    }
                    if !is_new_session {
                        continue;
                    }
                    // A fresh session finally has an id: link registry+board.
                    let mut gstate = state_file::load(&config.data_dir());
                    if let Some(w) = gstate.workers.iter_mut().find(|w| w.task_id == task2) {
                        w.session_id = session_id.clone();
                    }
                    let _ = state_file::save(&config.data_dir(), &gstate);
                    use hark_core::ports::SessionStore;
                    if let Ok(mut store) = SqliteStore::open(&config.data_dir().join("index.db")) {
                        if let Ok(current) = store.board() {
                            let updates = [hark_core::domain::board::BoardUpdate {
                                titulo: board_title.clone(),
                                status: hark_core::domain::board::TaskStatus::Doing,
                                nota: None,
                                sessao: None,
                            }];
                            let merged = hark_core::domain::board::apply_updates(
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
                ClaudeEvent::ToolUse { name, input } => {
                    // An APPROVED plan becomes the task's visible checklist.
                    if name == "ExitPlanMode" {
                        let plan = serde_json::from_str::<serde_json::Value>(&input)
                            .ok()
                            .and_then(|v| v.get("plan").and_then(|p| p.as_str()).map(String::from));
                        if let Some(plan) = plan {
                            let subs = hark_core::domain::board::plan_to_subtasks(&plan);
                            if !subs.is_empty() {
                                use hark_core::ports::SessionStore;
                                if let Ok(mut store) =
                                    SqliteStore::open(&config.data_dir().join("index.db"))
                                {
                                    if let Ok(mut tasks) = store.board() {
                                        if let Some(task) =
                                            tasks.iter_mut().find(|t| t.title == board_title)
                                        {
                                            task.subtasks = subs;
                                            task.updated_at = now_iso();
                                            let _ = store.save_board(&tasks);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    emit_event(
                        &app2,
                        serde_json::json!({ "kind": "worker", "task_id": task2, "name": name, "input": input }),
                    )
                }
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
                    app2.state::<PermLog>().0.lock().unwrap().push((
                        request_id.clone(),
                        board_title.clone(),
                        tool_name.clone(),
                    ));
                    // The production gate: a flagged ask NEVER auto-resolves
                    // (standing rules, permissive windows) — a human answers.
                    let prod_risk = hark_core::domain::prodgate::check(&tool_name, &input);
                    let _ = app2.emit(
                        "hark-permission",
                        serde_json::json!({
                            "request_id": request_id,
                            "task_id": task2,
                            "label": board_title,
                            "tool_name": tool_name,
                            "input": input,
                            "prod_risk": prod_risk,
                        }),
                    );
                }
                ClaudeEvent::Result(turn) => {
                    // The next turn starts from scratch: re-announce its
                    // first phase even if it matches this turn's last.
                    last_phase = None;
                    // Turn done, worker stays alive for the next message.
                    update_worker_summary(&config, &task2, &turn.raw);
                    if task2 != HARK_CHAT_TASK {
                        // (the chat's cwd is the data dir — no .hark there)
                        let _ = memory_files::append_state(
                            &workspace,
                            &format!("- {} {task2} [turn] {}", now_iso(), turn.raw.chars().take(160).collect::<String>()),
                        );
                    }
                    let workspace_str = workspace.display().to_string();
                    record_live_spend(
                        &config,
                        hark_core::domain::spend::SpendKind::Worker,
                        &hark_core::domain::spend::SpendMeta {
                            task_id: Some(&task2),
                            session_id: (!current_session.is_empty())
                                .then_some(current_session.as_str()),
                            workspace: Some(&workspace_str),
                            outcome: Some(&eco_outcome),
                            ..Default::default()
                        },
                        &turn,
                    );
                    // Aggregate usage + how full the context window is.
                    let mut usage = hark_plugin_claude::stream::TokenUsage::default();
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
                    // A failed turn carries its CLASS, so the windows can
                    // act (auth → open a terminal with `claude` typed) and
                    // know it is fatal — the crash-resend must never replay
                    // a message into a session that cannot authenticate.
                    let error_code = (turn.is_error
                        && hark_plugin_claude::health::looks_unauthenticated(&turn.raw))
                    .then_some("agent_auth");
                    emit_event(
                        &app2,
                        serde_json::json!({ "kind": "worker_turn", "task_id": task2,
                            "label": board_title,
                            "session_id": (!current_session.is_empty()).then_some(current_session.as_str()),
                            "error_code": error_code,
                            "text": turn.raw, "cost_usd": turn.cost_usd, "model": turn.model, "is_error": turn.is_error,
                            "usage": { "input": usage.input, "output": usage.output,
                                       "cache_read": usage.cache_read, "cache_created": usage.cache_created },
                            "context_pct": context_pct }),
                    );
                    // Batching: everything queued during this turn goes out
                    // now as ONE message; empty queue clears the flag.
                    let queued = {
                        let state = app2.state::<BatchState>();
                        let mut map = state.0.lock().unwrap();
                        match map.get_mut(&task2) {
                            Some(entry) if !entry.queue.is_empty() => {
                                Some(std::mem::take(&mut entry.queue).join("\n\n"))
                            }
                            Some(entry) => {
                                entry.in_flight = false;
                                None
                            }
                            None => None,
                        }
                    };
                    if let Some(joined) = queued {
                        let handle = app2.state::<LiveWorkers>().0.lock().unwrap().get(&task2).cloned();
                        if let Some(h) = handle {
                            let _ = h.session.send_text(&joined, &[]);
                            emit_event(
                                &app2,
                                serde_json::json!({ "kind": "status",
                                    "text": "fila entregue como uma mensagem" }),
                            );
                        }
                    }
                }
                ClaudeEvent::RateLimit(info) => emit_event(
                    &app2,
                    serde_json::json!({ "kind": "rate_limit", "status": info.status,
                        "resets_at": info.resets_at, "limit_kind": info.kind }),
                ),
                // What the agent is doing while it is quiet. A turn spends
                // most of a minute here — reasoning, or waiting on the API
                // — and the window used to show nothing at all until the
                // first finished block arrived.
                ClaudeEvent::Status(phase) if last_phase != Some(phase) => {
                    last_phase = Some(phase);
                    emit_event(
                        &app2,
                        serde_json::json!({ "kind": "phase",
                            "task_id": task2, "phase": phase }),
                    );
                }
                _ => {}
            }
        }
        // Only report the exit if nobody restarted this task meanwhile:
        // each spawn stamps a generation; this thread reports only its own.
        let handle = app2.state::<LiveWorkers>().0.lock().unwrap().get(&task2).cloned();
        let restarted = handle.as_ref().is_some_and(|h| h.generation != generation);
        if !restarted {
            // Why it died travels with the event: exit code + the CLI's
            // final words on stderr. "encerrado" alone was unactionable.
            let (exit_code, stderr_tail) = handle
                .as_ref()
                .map(|h| h.session.exit_report())
                .unwrap_or((None, String::new()));
            let reason: String = stderr_tail.chars().rev().take(300).collect::<Vec<_>>()
                .into_iter().rev().collect();
            // Classify the death so the windows can refuse a doomed
            // resend: an unauthenticated worker dies again identically.
            let reason = if hark_plugin_claude::health::looks_unauthenticated(&reason) {
                format!("agent_auth: {}", reason.trim())
            } else {
                reason.trim().to_string()
            };
            emit_event(
                &app2,
                serde_json::json!({ "kind": "worker_exit", "task_id": task2,
                    "exit_code": exit_code,
                    "reason": (!reason.is_empty()).then_some(reason.as_str()) }),
            );
            app2.state::<LiveWorkers>().0.lock().unwrap().remove(&task2);
            app2.state::<BatchState>().0.lock().unwrap().remove(&task2);
        }
    });

    Ok(())
}

/// Board title for a worker. A resumed session already HAS a name — the
/// words the user opened it with — and the instruction that reached it is
/// a terrible title, especially when dictated ("tinha um chat aberto de…").
/// Only a brand-new session falls back to the instruction.
fn worker_board_title(session_id: Option<&str>, instruction: &str) -> String {
    let from_session = session_id.filter(|s| !s.is_empty()).and_then(|id| {
        let config = Config::load();
        let store = SqliteStore::open(&config.data_dir().join("index.db")).ok()?;
        session_summary(&store, id).map(|s| session_label(&s))
    });
    from_session.unwrap_or_else(|| instruction.chars().take(60).collect())
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

    use hark_core::ports::SessionStore;
    if let Ok(mut store) = SqliteStore::open(&config.data_dir().join("index.db")) {
        if let Ok(current) = store.board() {
            let updates = [hark_core::domain::board::BoardUpdate {
                titulo: worker_board_title(session_id, instruction),
                status: hark_core::domain::board::TaskStatus::Doing,
                nota: Some("worker conversacional ativo".into()),
                sessao: None,
            }];
            let merged = hark_core::domain::board::apply_updates(
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
    mode: Option<String>,
    model: Option<String>,
) -> Result<DispatchOut, String> {
    let config = Config::load();
    let root = std::path::PathBuf::from(hark_core::config::expand_home(&project_path));
    if !root.is_dir() {
        return Err(format!("diretório não existe: {}", root.display()));
    }
    let task_id = format!("n-{}", Utc::now().format("%m%d%H%M%S"));
    update_registry_and_board(&config, &task_id, &root, None, &instruction, WorkerStatus::Running);

    // Mode precedence: spoken directive > window selector > config default.
    let mut directives = hark_core::domain::directives::parse(&instruction);
    directives.mode = directives
        .mode
        .or_else(|| mode.as_deref().and_then(hark_core::domain::directives::Mode::from_flag))
        .or_else(|| config.default_worker_mode());
    directives.model = directives.model.or(model.filter(|m| !m.is_empty()));
    let spawn = fresh_spawn(&config, root, String::new(), instruction, directives.clone());
    start_worker(&app, &live, &task_id, spawn).map_err(|e| e.to_string())?;
    Ok(DispatchOut::Started { task_id, directives })
}

/// Persist one live turn into the spend ledger. Failures never break the
/// flow: the ledger is bookkeeping, not the critical path.
fn record_live_spend(
    config: &Config,
    kind: hark_core::domain::spend::SpendKind,
    meta: &hark_core::domain::spend::SpendMeta,
    turn: &hark_plugin_claude::stream::TurnResult,
) {
    use hark_core::ports::SpendLedger;
    let rows = hark_core::domain::spend::rows_from_turn(&now_iso(), kind, meta, turn);
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
    images: Option<Vec<(String, String)>>,
) -> Result<hark_core::domain::directives::Directives, String> {
    let handle = live
        .0
        .lock()
        .unwrap()
        .get(&task_id)
        .cloned()
        .ok_or("worker não está mais ativo")?;

    // Batching (opt-in): a turn is running → queue, deliver as ONE message
    // when it ends. Queued text skips directive parsing by design.
    if Config::load().batch_messages {
        let state = app.state::<BatchState>();
        let mut map = state.0.lock().unwrap();
        let entry = map.entry(task_id.clone()).or_default();
        if entry.in_flight {
            entry.queue.push(text.clone());
            let n = entry.queue.len();
            drop(map);
            emit_event(
                &app,
                serde_json::json!({ "kind": "status",
                    "text": format!("turno em andamento — mensagem na fila ({n} pendente(s))") }),
            );
            return Ok(handle.spec.directives.clone());
        }
        entry.in_flight = true;
    }

    let asked = hark_core::domain::directives::parse(&text);
    let mut next = handle.spec.directives.clone();
    if asked.mode.is_some() {
        next.mode = asked.mode;
    }
    if asked.effort.is_some() {
        next.effort = asked.effort;
    }
    if asked.model.is_some() {
        next.model = asked.model.clone();
    }

    if next == handle.spec.directives {
        handle
            .session
            .send_text(&text, &images.unwrap_or_default())
            .map_err(|e| e.to_string())?;
        return Ok(next);
    }

    // Directive changed: only a fresh process can carry new CLI flags.
    emit_event(
        &app,
        serde_json::json!({ "kind": "status",
            "text": format!("reabrindo a thread com {}", describe(&next)) }),
    );
    handle.session.shutdown();
    // Limits carry over from the original spec (`..clone()`).
    let spec = SessionSpec {
        instruction: text,
        directives: next.clone(),
        ..handle.spec.clone()
    };
    start_worker(&app, &live, &task_id, spec).map_err(|e| e.to_string())?;
    Ok(next)
}

/// The mother's persistent work chat: one task id, off the board, off the
/// worker registry. Its session id lives in the global state so the chat
/// survives app restarts.
const HARK_CHAT_TASK: &str = "hark-chat";

/// Opt-in message batching (`batch_messages = true`): follow-ups sent
/// while a turn is in flight queue up and land as ONE message when the
/// turn ends — fewer, fatter turns re-read the cached context less often.
#[derive(Default)]
pub(crate) struct BatchState(pub(crate) Mutex<HashMap<String, BatchEntry>>);
#[derive(Default)]
pub(crate) struct BatchEntry {
    pub(crate) in_flight: bool,
    pub(crate) queue: Vec<String>,
}

/// Tick/untick one step of a task's plan checklist.
#[tauri::command]
fn board_subtask_toggle(title: String, index: usize, done: bool) -> Result<(), String> {
    use hark_core::ports::SessionStore;
    let config = Config::load();
    let mut store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let mut tasks = store.board().map_err(|e| e.to_string())?;
    let task = tasks
        .iter_mut()
        .find(|t| t.title == title)
        .ok_or("task não está no quadro")?;
    let sub = task.subtasks.get_mut(index).ok_or("passo fora do checklist")?;
    sub.done = done;
    task.updated_at = now_iso();
    store.save_board(&tasks).map_err(|e| e.to_string())
}

/// Per-worker eco-tool env vars from what the machine has + `[assist]`.
/// The ONE way to build a fresh worker spawn. Everything config-derived
/// (binary path, hard caps, eco envs) comes from here, so the four spawn
/// paths can never drift apart again; restarts derive from the original
/// spawn with `..clone()` and inherit the same pieces.
fn fresh_spawn(
    config: &Config,
    cwd: std::path::PathBuf,
    session_id: String,
    instruction: String,
    directives: hark_core::domain::directives::Directives,
) -> SessionSpec {
    SessionSpec {
        agent: "claude".into(),
        // The project's own ceiling when configured — the cap follows the
        // workspace the worker actually runs in (FASE 8.6).
        limits: config.spawn_limits_for(&cwd),
        envs: worker_envs(config),
        cwd,
        session_id,
        instruction,
        directives,
        fork: false,
    }
}

pub(crate) fn worker_envs(config: &Config) -> Vec<(String, String)> {
    let settings = std::fs::read_to_string(hark_core::config::expand_home("~/.claude/settings.json"))
        .unwrap_or_default();
    let status = hark_plugin_claude::eco::detect(&settings);
    hark_plugin_claude::eco::eco_envs(
        status,
        config.assist.ponytail.as_deref(),
        config.assist.caveman.as_deref(),
        config.assist.tokensave.as_deref(),
    )
}

/// The ledger fingerprint of a spawn's eco set (feeds spend.outcome).
pub(crate) fn eco_fingerprint(envs: &[(String, String)]) -> String {
    let settings = std::fs::read_to_string(hark_core::config::expand_home("~/.claude/settings.json"))
        .unwrap_or_default();
    let status = hark_plugin_claude::eco::detect(&settings);
    hark_plugin_claude::eco::fingerprint(status, envs)
}

/// What eco tools this machine has and how workers run them.
#[tauri::command(async)]
fn eco_status() -> Result<serde_json::Value, String> {
    let config = Config::load();
    let settings = std::fs::read_to_string(hark_core::config::expand_home("~/.claude/settings.json"))
        .unwrap_or_default();
    let status = hark_plugin_claude::eco::detect(&settings);
    let envs = worker_envs(&config);
    Ok(serde_json::json!({
        "status": status,
        "envs": envs,
        "fingerprint": hark_plugin_claude::eco::fingerprint(status, &envs),
    }))
}

/// Is the Claude Code binary actually reachable? Returns the resolved path
/// alongside the verdict — the wizard and the plugin catalog both need it.
fn claude_detected(config: &Config) -> (bool, String) {
    let bin = config.claude_bin_resolved();
    let ok = std::path::Path::new(&bin).is_absolute()
        || std::process::Command::new("which")
            .arg(&bin)
            .output()
            .is_ok_and(|out| out.status.success());
    (ok, bin)
}

/// The agent-plugin catalog: which backends exist, which one drives the
/// sessions, and what each can do — the capability sheet comes straight
/// from the plugin's own declaration, so it can never drift from the code.
#[tauri::command]
fn agent_plugins() -> Result<serde_json::Value, String> {
    let config = Config::load();
    let selected: &str =
        if config.agent.plugin.is_empty() { "claude" } else { &config.agent.plugin };
    let (claude_ok, claude_bin) = claude_detected(&config);

    Ok(serde_json::json!([
        {
            "id": "claude",
            "name": "Claude Code",
            "crate_name": "hark-plugin-claude",
            "vendor": "Anthropic",
            "status": "available",
            "detected": claude_ok,
            "detail": if claude_ok { claude_bin } else { String::new() },
            "install": "curl -fsSL https://claude.ai/install.sh | bash",
            "selected": selected == "claude",
            "capabilities": hark_plugin_claude::capabilities(),
        },
        {
            "id": "gemini",
            "name": "Gemini CLI",
            "crate_name": "hark-plugin-gemini",
            "vendor": "Google",
            "status": "planned",
            "detected": false,
            "detail": "",
            "install": "",
            "selected": false,
            "capabilities": serde_json::Value::Null,
        }
    ]))
}

/// Pick the agent plugin that drives new sessions. An unknown or unshipped
/// id is refused here, so the UI can never point the app at a backend that
/// cannot run.
#[tauri::command]
fn agent_plugin_select(id: String) -> Result<(), String> {
    if id != "claude" {
        return Err(format!("plugin indisponivel: {id}"));
    }
    let path = hark_core::config::config_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let out = hark_core::config::patch_toml(&text, &serde_json::json!({ "agent.plugin": id }))
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, out).map_err(|e| e.to_string())
}

/// Everything the first-run wizard needs to decide what to show: which
/// pieces exist (config, whisper model, claude binary, history) and the
/// current values to pre-fill.
#[tauri::command]
fn setup_status() -> Result<serde_json::Value, String> {
    let config = Config::load();
    let whisper = config.whisper_model_path();
    let (claude_ok, claude) = claude_detected(&config);
    let state = hark_core::adapters::state_file::load(&config.data_dir());
    // "Recommended" depends on the machine, not on the catalog: without
    // Metal (Intel) the strong model transcribes at ~3x real time and the
    // mic feels frozen — there the fast model is the honest default.
    let recommended = hark_core::domain::speech_model::recommended_stt_model(
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    let models: Vec<serde_json::Value> = hark_core::adapters::model_fetch::whisper_models()
        .iter()
        .map(|m| {
            serde_json::json!({
                "key": m.key,
                "filename": m.filename,
                "size_label": m.size_label,
                "recommended": m.key == recommended,
            })
        })
        .collect();
    Ok(serde_json::json!({
        "onboarded": state.onboarded,
        "config_exists": hark_core::config::config_path().exists(),
        "whisper_ok": whisper.exists(),
        "whisper_path": whisper.display().to_string(),
        "claude_bin": claude,
        "claude_ok": claude_ok,
        "projects_dir_ok": config.projects_dir.exists(),
        "models": models,
        // Non-null only when the recommendation is a downgrade the wizard
        // should justify ("no_metal_cpu" on Intel).
        "stt_reco_reason": hark_core::domain::speech_model::recommendation_reason(
            std::env::consts::OS,
            std::env::consts::ARCH,
        ),
        "language": config.language,
        "assistant_name": config.assistant_name,
        "hotkey": config.hotkey,
    }))
}

/// Download a whisper model in the background, streaming progress to the
/// window as `hark-setup` events ({key, pct, done, error}). On success the
/// config points at the downloaded file explicitly.
#[tauri::command]
fn setup_download_model(app: AppHandle, key: String) -> Result<(), String> {
    let model = hark_core::adapters::model_fetch::whisper_model(&key)
        .ok_or_else(|| format!("modelo desconhecido: {key}"))?;
    let config = Config::load();
    let dir = config.data_dir().join("models");
    std::thread::spawn(move || {
        let emit = |payload: serde_json::Value| {
            let _ = app.emit("hark-setup", payload);
        };
        let mut last = 255u8;
        let result = hark_core::adapters::model_fetch::download_model(model, &dir, |pct| {
            if pct != last {
                last = pct;
                emit(serde_json::json!({ "key": model.key, "pct": pct }));
            }
        });
        match result {
            Ok(path) => {
                // Point the config at the verified file so a non-default
                // choice (large-v3-turbo) is what the mic actually loads.
                let patch =
                    serde_json::json!({ "whisper_model": path.display().to_string() });
                let text =
                    std::fs::read_to_string(hark_core::config::config_path()).unwrap_or_default();
                if let Ok(out) = hark_core::config::patch_toml(&text, &patch) {
                    if let Some(parent) = hark_core::config::config_path().parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::write(hark_core::config::config_path(), out);
                }
                // Warm whisper so the mic works right after the wizard.
                ensure_speech_loading();
                emit(serde_json::json!({ "key": model.key, "pct": 100, "done": true }));
            }
            Err(err) => {
                emit(serde_json::json!({ "key": model.key, "error": err.to_string() }));
            }
        }
    });
    Ok(())
}

/// The wizard finished (or was skipped): never auto-open it again.
#[tauri::command]
fn setup_mark_done() -> Result<(), String> {
    let config = Config::load();
    let data_dir = config.data_dir();
    let mut state = hark_core::adapters::state_file::load(&data_dir);
    state.onboarded = true;
    hark_core::adapters::state_file::save(&data_dir, &state).map_err(|e| e.to_string())
}

/// The soul file: <data_dir>/HARK.md, agent-neutral and canonical. Each
/// CLI auto-loads its own memory name (CLAUDE.md, GEMINI.md…), so those
/// exist as symlinks to the soul — one persona, one learning history,
/// whatever agent is talking. Created once from the template; after that
/// it belongs to the user and to the model's own edits — NEVER overwritten.
pub(crate) fn ensure_persona(config: &Config) {
    // One soul, many mouths: HARK.md is canonical; each agent's memory
    // file (the name its CLI auto-loads — capabilities().memory_file) is a
    // symlink to it, so Claude and a future Gemini share the same persona
    // and the same accumulated learnings.
    let dir = config.data_dir();
    let mouths: Vec<String> = [hark_plugin_claude::capabilities().memory_file]
        .into_iter()
        .flatten()
        .collect();
    let mouth_refs: Vec<&str> = mouths.iter().map(String::as_str).collect();
    hark_core::adapters::memory_files::ensure_soul_links(&dir, "HARK.md", &mouth_refs);
    let path = dir.join("HARK.md");
    if path.exists() {
        // One exception to "never overwritten": the lines that name the
        // assistant. The vox → hark rename moved the data dir and left
        // this file behind, so the soul kept saying "Você é Vox … no Vox"
        // and the assistant introduced itself by a product name that no
        // longer exists. Learnings are copied through untouched.
        if let Ok(doc) = std::fs::read_to_string(&path) {
            if let Some(fixed) =
                hark_core::domain::persona::retitle(&doc, &config.assistant_name)
            {
                let _ = std::fs::write(&path, fixed);
            }
        }
        return;
    }
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        &path,
        hark_core::domain::persona::template(&config.assistant_name, config.lang()),
    );
    // First boot: the canonical was just seeded; give the agents their links.
    hark_core::adapters::memory_files::ensure_soul_links(&dir, "HARK.md", &mouth_refs);
}

#[derive(Serialize)]
struct SavingsOut {
    avoided_gate_usd: f64,
    avoided_local_usd: f64,
    avoided_cache_usd: f64,
    total_usd: f64,
    methodology: Vec<String>,
}

/// The savings meter: ledger-derived inputs → domain::savings::compute.
/// Every number's formula travels in `methodology` — the UI shows it.
#[tauri::command(async)]
fn savings_summary(since: Option<String>) -> Result<SavingsOut, String> {
    use hark_core::domain::spend::SpendSource;
    use hark_core::ports::{SpendGroup, SpendLedger, SpendQuery};
    let config = Config::load();
    let store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let by = |group: SpendGroup| {
        store.spend_summary(&SpendQuery {
            since: since.clone(),
            group,
            source: SpendSource::Live,
            workspace: None,
        })
    };
    let kinds = by(SpendGroup::Kind).map_err(|e| e.to_string())?;
    let models = by(SpendGroup::Model).map_err(|e| e.to_string())?;
    let agg = |key: &str| kinds.iter().find(|a| a.key == key);

    // Gate interceptions: rows whose outcome marks a dispatch that never
    // became a worker turn (meta_hark — talk about hark, not work).
    let gate_blocked = store
        .spend_rows(since.as_deref().unwrap_or("0"), 1_000_000)
        .map_err(|e| e.to_string())?
        .iter()
        .filter(|r| {
            // "gate:meta_vox" is the same outcome recorded before the rename.
            matches!(r.outcome.as_deref(), Some("gate:meta_hark" | "gate:meta_vox"))
        })
        .count() as u64;

    let avg = |cost: f64, turns: u64| if turns > 0 { cost / turns as f64 } else { 0.0 };
    let worker = agg("worker");
    let ask = agg("ask");
    let (total_cost, total_in) = kinds.iter().fold((0.0, 0u64), |(c, t), a| {
        (c + a.cost_usd, t + a.usage.input + a.usage.cache_read + a.usage.cache_created)
    });
    let inputs = hark_core::domain::savings::SavingsInputs {
        gate_blocked,
        gate_cost_usd: agg("gate").map(|a| a.cost_usd).unwrap_or(0.0),
        avg_worker_turn_usd: worker.map(|a| avg(a.cost_usd, a.turns)).unwrap_or(0.0),
        local_answers: models.iter().find(|a| a.key == "local").map(|a| a.turns).unwrap_or(0),
        avg_ask_usd: ask.map(|a| avg(a.cost_usd, a.turns)).unwrap_or(0.0),
        cache_read_tokens: kinds.iter().map(|a| a.usage.cache_read).sum(),
        usd_per_input_token: if total_in > 0 { total_cost / total_in as f64 } else { 0.0 },
    };
    let report = hark_core::domain::savings::compute(&inputs);
    Ok(SavingsOut {
        avoided_gate_usd: report.avoided_gate_usd,
        avoided_local_usd: report.avoided_local_usd,
        avoided_cache_usd: report.avoided_cache_usd,
        total_usd: report.total_usd,
        methodology: report.methodology,
    })
}

/// Which surface a message to global hark belongs to: "lean" (bare one-shot
/// ask over the snapshot) or "work" (the persistent chat with full
/// settings + MCP). Pure domain passthrough, zero tokens.
#[tauri::command]
fn ask_lane(text: String) -> &'static str {
    match hark_core::domain::answer::lane(&text) {
        hark_core::domain::answer::AskLane::Lean => "lean",
        hark_core::domain::answer::AskLane::WorkChat => "work",
    }
}

#[derive(Serialize)]
struct HarkChatOut {
    task_id: String,
    /// True when this message continues a stored session (live or resumed).
    resumed: bool,
}

/// Send a message to the mother's work chat. Reuses the live worker when
/// there is one (directive changes restart it, same as any worker);
/// otherwise spawns it in the DATA DIR (neutral cwd, full user settings —
/// MCP and tools work) resuming the stored session when it still exists.
#[tauri::command(async)]
fn hark_chat_send(
    app: AppHandle,
    live: State<'_, LiveWorkers>,
    text: String,
    images: Option<Vec<(String, String)>>,
) -> Result<HarkChatOut, String> {
    let alive = live.0.lock().unwrap().contains_key(HARK_CHAT_TASK);
    if alive {
        worker_send(app, live, HARK_CHAT_TASK.into(), text, images)?;
        return Ok(HarkChatOut {
            task_id: HARK_CHAT_TASK.into(),
            resumed: true,
        });
    }

    let config = Config::load();
    // Resume only a session whose log file still exists; otherwise start
    // fresh (a stale id would make the spawn die silently).
    let stored = state_file::load(&config.data_dir()).hark_chat_session;
    let session = stored.filter(|id| {
        SqliteStore::open(&config.data_dir().join("index.db"))
            .ok()
            .and_then(|store| {
                use hark_core::ports::SessionStore;
                store.session_path(id).ok().flatten()
            })
            .is_some_and(|path| std::path::Path::new(&path).exists())
    });

    ensure_persona(&config);
    let mut directives = hark_core::domain::directives::parse(&text);
    directives.mode = directives.mode.or_else(|| config.default_worker_mode());
    let resumed = session.is_some();
    let mut spawn = fresh_spawn(
        &config,
        config.data_dir(),
        session.unwrap_or_default(),
        text,
        directives,
    );
    // The budget ceiling exists for dispatched workers, whose runaway is
    // the risk it guards. The assistant is the product's front door and
    // long-lived by design: with the cap it died at the $2 process line,
    // MID-TURN, showing "worker encerrado" and eating the message. Its
    // spend stays visible in the chat header; the ceiling does not apply.
    spawn.limits = hark_agent::SpawnLimits::default();
    start_worker(&app, &live, HARK_CHAT_TASK, spawn).map_err(|e| e.to_string())?;
    Ok(HarkChatOut {
        task_id: HARK_CHAT_TASK.into(),
        resumed,
    })
}

#[derive(Serialize)]
struct HarkChatStatus {
    alive: bool,
    session_id: Option<String>,
}

/// Is the mother's chat worker running, and which session backs it?
#[tauri::command]
fn hark_chat_status(live: State<'_, LiveWorkers>) -> HarkChatStatus {
    let config = Config::load();
    HarkChatStatus {
        alive: live.0.lock().unwrap().contains_key(HARK_CHAT_TASK),
        session_id: state_file::load(&config.data_dir()).hark_chat_session,
    }
}

/// Heavy session → fresh one on the SAME task/board card, context rebuilt
/// LOCALLY (zero tokens): the old transcript becomes a brief that opens
/// the new session. Always behind a click.
#[tauri::command(async)]
fn worker_restart_light(
    app: AppHandle,
    live: State<'_, LiveWorkers>,
    task_id: String,
) -> Result<(), String> {
    let handle = live
        .0
        .lock()
        .unwrap()
        .get(&task_id)
        .cloned()
        .ok_or("worker não está mais ativo")?;
    let config = Config::load();
    // The session actually backing this task now (registry beats spawn).
    let session = state_file::load(&config.data_dir())
        .workers
        .iter()
        .find(|w| w.task_id == task_id)
        .map(|w| w.session_id.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| handle.spec.session_id.clone());
    let brief = {
        use hark_core::ports::SessionStore;
        let store =
            SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
        let path = store
            .session_path(&session)
            .ok()
            .flatten()
            .ok_or("sessão sem arquivo indexado")?;
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let entries = hark_core::domain::transcript::tail_entries(text.lines(), 400);
        hark_core::domain::transcript::brief(&entries, 1500)
    };
    let title = worker_board_title(Some(&session), &handle.spec.instruction);
    emit_event(
        &app,
        serde_json::json!({ "kind": "status",
            "text": format!("recomeçando \"{title}\" leve: sessão nova com resumo local") }),
    );
    handle.session.shutdown();
    let spec = SessionSpec {
        session_id: String::new(),
        instruction: format!(
            "Contexto local da conversa anterior (resumo gerado sem custo):\n{brief}\n\nContinue o trabalho de onde paramos."
        ),
        ..handle.spec.clone()
    };
    start_worker_titled(&app, &live, &task_id, spec, Some(title)).map_err(|e| e.to_string())
}

fn describe(d: &hark_core::domain::directives::Directives) -> String {
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

/// Outcome of a mode switch: whether the process was actually reopened.
/// `restarted: false` means the flag is stored for the next spawn and the
/// window has to cover this turn's permissions itself.
#[derive(Serialize)]
struct SetModeOut {
    directives: hark_core::domain::directives::Directives,
    restarted: bool,
}

/// Switch a live worker's permission mode WITHOUT sending a message: the
/// UI selector. Flags are per-process, so this restarts the worker on the
/// same session (context re-cached, no text reaches the model) — but never
/// while a turn is running.
#[tauri::command]
fn worker_set_mode(
    app: AppHandle,
    live: State<'_, LiveWorkers>,
    task_id: String,
    mode: String,
) -> Result<SetModeOut, String> {
    let Some(mode) = hark_core::domain::directives::Mode::from_flag(&mode) else {
        return Err(format!("modo desconhecido: {mode}"));
    };
    let handle = live
        .0
        .lock()
        .unwrap()
        .get(&task_id)
        .cloned()
        .ok_or("worker não está mais ativo")?;
    let mut next = handle.spec.directives.clone();
    if next.mode == Some(mode) {
        return Ok(SetModeOut { directives: next, restarted: false });
    }
    next.mode = Some(mode);
    // A turn in flight OWNS the process. Reopening it here destroyed a
    // turn with seven tool calls in it: the shutdown cut the transport
    // mid-call, and the resume landed on a transcript whose last entry was
    // an unanswered tool_use, so the new process died on arrival too. The
    // flag waits for the next spawn; the window relaxes this turn's
    // permissions on its own, which is what the user actually asked for.
    let in_flight = app
        .state::<BatchState>()
        .0
        .lock()
        .unwrap()
        .get(&task_id)
        .is_some_and(|e| e.in_flight);
    if in_flight {
        // Nothing is stored here on purpose: the window owns the intent and
        // re-applies it the moment the turn lands, when a restart is free
        // of consequence.
        emit_event(
            &app,
            serde_json::json!({ "kind": "status",
                "text": format!("{} — sem reabrir a thread: o turno em voo continua",
                    describe(&next)) }),
        );
        return Ok(SetModeOut { directives: next, restarted: false });
    }
    emit_event(
        &app,
        serde_json::json!({ "kind": "status",
            "text": format!("reabrindo a thread com {}", describe(&next)) }),
    );
    handle.session.shutdown();
    let spec = SessionSpec {
        directives: next.clone(),
        ..handle.spec.clone()
    };
    start_worker(&app, &live, &task_id, spec).map_err(|e| e.to_string())?;
    Ok(SetModeOut { directives: next, restarted: true })
}

/// Switch the focused task's MODEL: same contract as worker_set_mode —
/// flags are per-process, so an idle thread reopens on the same session;
/// a turn in flight is never interrupted (the model applies on the next
/// reopen — unlike modes, the window cannot emulate a model change).
#[tauri::command]
fn worker_set_model(
    app: AppHandle,
    live: State<'_, LiveWorkers>,
    task_id: String,
    model: String,
) -> Result<SetModeOut, String> {
    let handle = live
        .0
        .lock()
        .unwrap()
        .get(&task_id)
        .cloned()
        .ok_or("worker não está mais ativo")?;
    let mut next = handle.spec.directives.clone();
    let target = (!model.is_empty()).then_some(model);
    if next.model == target {
        return Ok(SetModeOut { directives: next, restarted: false });
    }
    next.model = target;
    let in_flight = app
        .state::<BatchState>()
        .0
        .lock()
        .unwrap()
        .get(&task_id)
        .is_some_and(|e| e.in_flight);
    if in_flight {
        emit_event(
            &app,
            serde_json::json!({ "kind": "status",
                "text": "modelo trocado — vale na próxima abertura da thread (o turno em voo continua)" }),
        );
        return Ok(SetModeOut { directives: next, restarted: false });
    }
    emit_event(
        &app,
        serde_json::json!({ "kind": "status",
            "text": format!("reabrindo a thread com {}", describe(&next)) }),
    );
    handle.session.shutdown();
    let spec = SessionSpec { directives: next.clone(), ..handle.spec.clone() };
    start_worker(&app, &live, &task_id, spec).map_err(|e| e.to_string())?;
    Ok(SetModeOut { directives: next, restarted: true })
}

/// End the conversation: EOF + kill, board moves to waiting.
#[tauri::command]
fn worker_stop(live: State<'_, LiveWorkers>, task_id: String) -> Result<(), String> {
    let worker = live.0.lock().unwrap().remove(&task_id).ok_or("worker não encontrado")?;
    worker.session.shutdown();
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
        "# Hark dispatch {task_id}\n\n- session: {}\n- workspace: {}\n\n## Instruction\n\n{instruction}\n",
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

    // The one-shot path honors the config default mode too.
    let one_shot_directives = {
        let mut d = hark_core::domain::directives::parse(&instruction);
        d.mode = d.mode.or_else(|| config.default_worker_mode());
        d
    };
    let spawn = fresh_spawn(
        &config,
        planned.workspace_root.clone(),
        planned.session.session_id.clone(),
        instruction.clone(),
        one_shot_directives,
    );
    let label = worker_board_title(Some(&planned.session.session_id), &instruction);
    // One-shot on the same backend seam as persistent workers: consume the
    // event stream inline, block on permission clicks, stop at the result.
    let result: anyhow::Result<hark_agent::TurnResult> = (|| {
        let backend = backend_for(&config);
        let (session, events) = backend.spawn(&spawn)?;
        let mut outcome: Option<hark_agent::TurnResult> = None;
        for event in events.iter() {
            match event {
                ClaudeEvent::ToolUse { name, input } => {
                    emit_event(
                        &app,
                        serde_json::json!({ "kind": "worker", "task_id": task_id, "name": name, "input": input }),
                    );
                }
                ClaudeEvent::PermissionRequest { request_id, tool_name, input } => {
                    // Surface in the UI and block this thread on the click.
                    let (tx, rx) = mpsc::channel();
                    pending.0.lock().unwrap().insert(request_id.clone(), tx);
                    app.state::<PermLog>().0.lock().unwrap().push((
                        request_id.clone(),
                        label.to_string(),
                        tool_name.clone(),
                    ));
                    let _ = app.emit(
                        "hark-permission",
                        serde_json::json!({
                            "request_id": request_id,
                            "task_id": task_id,
                            "label": label,
                            "tool_name": tool_name,
                            "input": input,
                        }),
                    );
                    let decision = rx
                        .recv_timeout(std::time::Duration::from_secs(300))
                        .unwrap_or(PermissionDecision::Deny);
                    let _ = session.respond_permission(&request_id, decision);
                }
                ClaudeEvent::Result(turn) => {
                    outcome = Some(turn);
                    break;
                }
                _ => {}
            }
        }
        session.shutdown();
        outcome.ok_or_else(|| {
            let (code, stderr) = session.exit_report();
            let status = code.map_or_else(|| "?".to_string(), |c| c.to_string());
            anyhow::anyhow!(hark_plugin_claude::health::exit_error("agent", &status, &stderr))
        })
    })();

    // Ledger before anything else: even failed turns burned tokens.
    if let Ok(turn) = &result {
        let workspace_str = planned.workspace_root.display().to_string();
        record_live_spend(
            &config,
            hark_core::domain::spend::SpendKind::Dispatch,
            &hark_core::domain::spend::SpendMeta {
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

impl From<hark_core::ports::SpendAgg> for SpendAggOut {
    fn from(a: hark_core::ports::SpendAgg) -> Self {
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

fn spend_group(group: &str) -> hark_core::ports::SpendGroup {
    use hark_core::ports::SpendGroup;
    match group {
        "model" => SpendGroup::Model,
        "label" => SpendGroup::Label,
        "workspace" => SpendGroup::Workspace,
        "day" => SpendGroup::Day,
        "session" => SpendGroup::Session,
        "task" => SpendGroup::Task,
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
    workspace: Option<String>,
) -> Result<Vec<SpendAggOut>, String> {
    use hark_core::ports::{SpendLedger, SpendQuery};
    let config = Config::load();
    let store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let query = SpendQuery {
        since,
        group: spend_group(&group),
        source: if source == "jsonl" {
            hark_core::domain::spend::SpendSource::Jsonl
        } else {
            hark_core::domain::spend::SpendSource::Live
        },
        // The project window scopes every number to its own directory.
        workspace: workspace.map(|w| hark_core::config::expand_home(&w)),
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
    use hark_core::ports::{SessionStore, SpendLedger};
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
                    agg.key = friendly_session_name(&title);
                }
            }
        }
    }
    Ok(aggs)
}

/// Ask sessions are titled by their own prompt, which starts with the
/// snapshot header — useless on a list of expensive sessions. Everything
/// else keeps its real title, clipped.
fn friendly_session_name(title: &str) -> String {
    if let Some(rest) = title.strip_prefix("Contexto gerado em: ") {
        let when = rest.get(..16).unwrap_or(rest).replace('T', " ");
        return format!("pergunta ao hark · {when}");
    }
    title.chars().take(48).collect()
}

#[derive(Serialize)]
struct ContextWeight {
    /// input + cache_read + cache_created of the session's last live turn.
    last_total_tokens: u64,
    /// The same total split into its parts, so the UI can draw the window
    /// like the CLI does: reused cache, fresh cache, new prompt.
    cache_read: u64,
    cache_created: u64,
    input: u64,
    output: u64,
    context_window: Option<u64>,
    pct: Option<f64>,
}

/// How full a session's context window is, from its last live turn.
#[tauri::command(async)]
fn session_context_weight(session_id: String) -> Result<ContextWeight, String> {
    let config = Config::load();
    let store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let w = store
        .last_context_weight(&session_id)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    Ok(ContextWeight {
        last_total_tokens: w.total,
        cache_read: w.cache_read,
        cache_created: w.cache_created,
        input: w.input,
        output: w.output,
        context_window: w.context_window,
        pct: w
            .context_window
            .filter(|c| *c > 0)
            .map(|c| (w.total as f64 / c as f64).min(1.0)),
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
    use hark_core::ports::SessionStore;
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

/// Sessions a human is holding at a terminal, ours or anyone's.
///
/// `claude agents --json` costs a subprocess, and every window asks the
/// same question while a takeover is on screen, so the answer is shared
/// for a beat. The listing is the whole signal: a session that ended
/// leaves it.
#[tauri::command(async)]
fn session_owners(
    live: State<'_, LiveWorkers>,
) -> Result<Vec<hark_core::domain::owner::Owner>, String> {
    use hark_core::ports::LiveSessions;
    type OwnersCache =
        std::sync::Mutex<Option<(std::time::Instant, Vec<hark_core::domain::owner::Owner>)>>;
    static CACHE: std::sync::OnceLock<OwnersCache> = std::sync::OnceLock::new();
    const TTL: std::time::Duration = std::time::Duration::from_millis(1200);

    // Sessions Hark is driving itself are not takeovers.
    let ours: Vec<String> = live
        .0
        .lock()
        .unwrap()
        .values()
        .map(|h| h.spec.session_id.clone())
        .filter(|s| !s.is_empty())
        .collect();

    let cell = CACHE.get_or_init(|| std::sync::Mutex::new(None));
    let mut slot = cell.lock().unwrap();
    if let Some((at, owners)) = slot.as_ref() {
        if at.elapsed() < TTL {
            let cached = owners.clone();
            return Ok(cached
                .into_iter()
                .filter(|o| !ours.contains(&o.session_id))
                .collect());
        }
    }
    let config = Config::load();
    let listing = hark_plugin_claude::live::ClaudeAgentsCli {
        claude_bin: config.claude_bin_resolved(),
    }
    .list()
    .unwrap_or_default();
    let owners = hark_core::domain::owner::terminal_owners(&listing, &ours);
    *slot = Some((std::time::Instant::now(), owners.clone()));
    Ok(owners)
}

/// Git state for a set of task workspaces, in one call.
///
/// Zero tokens: three `git` reads per repository, cached for a couple of
/// seconds so a re-render never shells out again. Directories that are
/// not repositories (or that git cannot reach) are simply absent from
/// the map — the UI shows nothing rather than a wrong branch.
#[tauri::command(async)]
fn repo_states(
    paths: Vec<String>,
) -> Result<std::collections::HashMap<String, hark_core::domain::repo::RepoState>, String> {
    type StateCache = std::sync::Mutex<
        std::collections::HashMap<
            String,
            (std::time::Instant, Option<hark_core::domain::repo::RepoState>),
        >,
    >;
    static CACHE: std::sync::OnceLock<StateCache> = std::sync::OnceLock::new();
    const TTL: std::time::Duration = std::time::Duration::from_secs(3);

    let cell = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let mut out = std::collections::HashMap::new();
    // Distinct paths only: several tasks commonly share one workspace.
    let mut seen: Vec<String> = Vec::new();
    for path in paths {
        if path.is_empty() || seen.contains(&path) {
            continue;
        }
        seen.push(path.clone());
        let fresh = {
            let slot = cell.lock().unwrap();
            match slot.get(&path) {
                Some((at, state)) if at.elapsed() < TTL => Some(state.clone()),
                _ => None,
            }
        };
        let state = match fresh {
            Some(state) => state,
            None => {
                let read = GitCli::work_tree(&path);
                cell.lock()
                    .unwrap()
                    .insert(path.clone(), (std::time::Instant::now(), read.clone()));
                read
            }
        };
        if let Some(state) = state {
            out.insert(path, state);
        }
    }
    Ok(out)
}

#[derive(Serialize)]
pub(crate) struct CrossrefOut {
    pub(crate) title: String,
    #[allow(dead_code)]
    pub(crate) session_id: String,
    /// The attributed, closed context block to travel inside the message.
    pub(crate) block: String,
    pub(crate) lines: usize,
}

/// Does this sentence cite ANOTHER conversation? If so, resolve it in the
/// local index and pull the lines about the sentence's subject. Zero
/// tokens: grammar + index + log file, all on this machine (FASE 8.2).
#[tauri::command(async)]
fn crossref_context(utterance: String) -> Result<Option<CrossrefOut>, String> {
    crossref_lookup(utterance)
}

/// The same lookup for in-process callers (voice_execute), free of the
/// command macro's namespace.
pub(crate) fn crossref_lookup(utterance: String) -> Result<Option<CrossrefOut>, String> {
    use hark_core::domain::{crossref, funnel, matching};
    use hark_core::ports::SessionStore;
    let Some(reference) = crossref::detect(&utterance) else {
        return Ok(None);
    };
    let hits = session_hits(&reference.query, 8)?;
    let leads: Vec<funnel::SessionLead> = hits
        .into_iter()
        .map(|h| funnel::SessionLead {
            session_id: h.session_id,
            title: h.title,
            cwd: h.cwd,
            last_ts: h.last_ts,
            last_prompt: h.last_prompt,
        })
        .collect();
    // A citation feeds context, it does not execute anything — so unlike a
    // dispatch target, an ambiguous best-match is acceptable and visible.
    let hit = match funnel::rank_sessions(&reference.query, leads) {
        matching::Match::Hit(h) => h,
        matching::Match::Ambiguous(mut hs) if !hs.is_empty() => hs.remove(0),
        _ => return Ok(None),
    };
    let config = Config::load();
    let store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let Some(path) = store.session_path(&hit.session_id).map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let entries: Vec<_> = content
        .lines()
        .filter_map(hark_core::domain::transcript::parse_entry)
        .collect();
    // Terms come from the WHOLE instruction: the excerpt is about what the
    // user is doing, not about the citation wording.
    let terms = matching::significant(&utterance);
    let excerpt = crossref::excerpt_about(&entries, &terms, 800);
    if excerpt.is_empty() {
        return Ok(None);
    }
    let lines = excerpt.lines().count();
    Ok(Some(CrossrefOut {
        block: crossref::context_block(&hit.title, &excerpt),
        title: hit.title,
        session_id: hit.session_id,
        lines,
    }))
}

#[derive(Serialize)]
struct MirrorOut {
    entries: Vec<hark_core::domain::transcript::Entry>,
    /// Bytes consumed so far — pass it back to read only what is new.
    offset: u64,
}

/// Follow a session's log from a byte offset: the terminal takeover's
/// mirror. The log is append-only and `--resume` writes to the SAME file
/// (verified), so reading what grew is the whole mechanism. Zero tokens,
/// no process, works no matter who is writing.
#[tauri::command(async)]
fn transcript_since(session_id: String, offset: u64) -> Result<MirrorOut, String> {
    use hark_core::ports::SessionStore;
    use std::io::{Read, Seek};
    let config = Config::load();
    let store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let path = store
        .session_path(&session_id)
        .map_err(|e| e.to_string())?
        .ok_or("sessão não está no índice")?;
    let mut file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
    let len = file.metadata().map_err(|e| e.to_string())?.len();
    // Truncated or replaced under us: start over rather than read garbage.
    let from = if offset > len { 0 } else { offset };
    file.seek(std::io::SeekFrom::Start(from)).map_err(|e| e.to_string())?;
    let mut buf = String::new();
    file.read_to_string(&mut buf).map_err(|e| e.to_string())?;
    // A line still being written has no newline yet: leave it for next time.
    let complete = buf.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let entries = buf[..complete]
        .lines()
        .filter_map(hark_core::domain::transcript::parse_entry)
        .collect();
    Ok(MirrorOut {
        entries,
        offset: from + complete as u64,
    })
}

#[derive(Serialize)]
struct TranscriptOut {
    /// The session's own title, as shown in Claude Code.
    session_title: Option<String>,
    entries: Vec<hark_core::domain::transcript::Entry>,
    /// Byte length of the log at read time: where a mirror starts from.
    offset: u64,
}

/// Read-only history of a past session, straight from its log file.
/// Costs nothing: no process spawned, no tokens.
#[tauri::command(async)]
fn read_transcript(session_id: String, limit: Option<usize>) -> Result<TranscriptOut, String> {
    use hark_core::ports::SessionStore;
    let config = Config::load();
    let store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let path = store
        .session_path(&session_id)
        .map_err(|e| e.to_string())?
        .ok_or("sessão não está no índice (rode: hark index)")?;
    let (session_title, _, _) = store
        .file_state(&path)
        .map_err(|e| e.to_string())?
        .map(|(s, o, m)| (s.title, o, m))
        .unwrap_or_default();
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(TranscriptOut {
        session_title,
        entries: hark_core::domain::transcript::tail_entries(content.lines(), limit.unwrap_or(200)),
        offset: content.len() as u64,
    })
}

/// Best session for a spoken/typed query, via the same topic search the
/// snapshot uses. Local and free.
#[tauri::command(async)]
fn find_session(query: String) -> Result<Option<serde_json::Value>, String> {
    use hark_core::ports::SessionStore;
    let config = Config::load();
    let mut store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let _ = hark_plugin_claude::history::refresh_index(&config.projects_dir, &mut store);
    let terms = hark_core::domain::dispatch::significant_terms(&query);
    let hits = store.search_sessions(&terms, 4).map_err(|e| e.to_string())?;
    let data_dir = config.data_dir().display().to_string();
    let hits: Vec<_> = hits
        .into_iter()
        .filter(|s| !hark_core::domain::owner::is_machinery_cwd(s.cwd.as_deref(), &data_dir))
        .collect();
    Ok(hits.first().map(|s| {
        // cwd travels too: it is how a board task with no workspace still
        // resolves to a project window.
        serde_json::json!({ "session_id": s.session_id, "title": s.title, "cwd": s.cwd })
    }))
}

/// One indexed session offered as a recovery candidate.
#[derive(Serialize, Clone)]
pub(crate) struct SessionHit {
    pub(crate) session_id: String,
    /// Never empty: real title, opening prompt, or the id itself.
    pub(crate) title: String,
    pub(crate) cwd: Option<String>,
    pub(crate) last_ts: Option<String>,
    pub(crate) last_prompt: Option<String>,
}

/// The indexed summary of one session (id → path → folded state).
pub(crate) fn session_summary(
    store: &SqliteStore,
    session_id: &str,
) -> Option<hark_core::domain::snapshot::SessionSummary> {
    use hark_core::ports::SessionStore;
    let path = store.session_path(session_id).ok().flatten()?;
    store.file_state(&path).ok().flatten().map(|(s, _, _)| s)
}

/// The name a human recognises this session by.
fn session_label(summary: &hark_core::domain::snapshot::SessionSummary) -> String {
    summary
        .title
        .as_deref()
        .or(summary.last_prompt.as_deref())
        .map(friendly_session_name)
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| summary.session_id.chars().take(8).collect())
}

/// Sessions matching a topic, straight from the local index. Recovering a
/// session someone remembers is a lookup, not an investigation: no agent,
/// no shell archaeology, zero tokens.
fn session_hits(query: &str, limit: usize) -> Result<Vec<SessionHit>, String> {
    use hark_core::ports::SessionStore;
    let config = Config::load();
    let mut store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    // The index only used to move when `ask` ran, so sessions opened since
    // the last question were invisible here. Incremental (mtime + byte
    // offset), so this is milliseconds when nothing changed.
    let _ = hark_plugin_claude::history::refresh_index(&config.projects_dir, &mut store);
    let terms = hark_core::domain::dispatch::significant_terms(query);
    // Overfetch: Hark's own machinery sessions (cwd inside ~/.hark — ask
    // one-shots, the hark-chat worker) are filtered out below, and they
    // match aggressively whenever the query quotes a Hark answer.
    let hits = store
        .search_sessions(&terms, limit * 2)
        .map_err(|e| e.to_string())?;
    let data_dir = config.data_dir().display().to_string();
    Ok(hits
        .into_iter()
        .filter(|s| !hark_core::domain::owner::is_machinery_cwd(s.cwd.as_deref(), &data_dir))
        .take(limit)
        .map(|s| SessionHit {
            title: session_label(&s),
            session_id: s.session_id,
            cwd: s.cwd,
            last_ts: s.last_ts,
            last_prompt: s.last_prompt.map(|p| p.chars().take(120).collect()),
        })
        .collect())
}

/// Search sessions by topic on demand (the "recuperar sessão" picker).
#[tauri::command(async)]
fn session_candidates(query: String, limit: Option<usize>) -> Result<Vec<SessionHit>, String> {
    session_hits(&query, limit.unwrap_or(6))
}

/// Every Claude Code session of one project, newest first, straight from
/// the local index. This is what lets the sidebar show the full history —
/// the board stays the layer of intent on top of it. Zero tokens.
#[tauri::command(async)]
fn project_sessions(path: String) -> Result<Vec<SessionHit>, String> {
    use hark_core::ports::SessionStore;
    let config = Config::load();
    let mut store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let _ = hark_plugin_claude::history::refresh_index(&config.projects_dir, &mut store);
    let root = hark_core::config::expand_home(&path);
    let all = store.sessions_since("0").map_err(|e| e.to_string())?;
    Ok(all
        .into_iter()
        .filter(|s| {
            s.cwd
                .as_deref()
                .is_some_and(|c| c == root || c.starts_with(&format!("{root}/")))
        })
        .map(|s| SessionHit {
            title: session_label(&s),
            session_id: s.session_id,
            cwd: s.cwd,
            last_ts: s.last_ts,
            last_prompt: s.last_prompt.map(|p| p.chars().take(120).collect()),
        })
        .collect())
}

/// Turn a recovered session into a board task named after the SESSION —
/// never after the sentence that found it — and bind the two, so the chat
/// and the card are the same thing from here on.
#[tauri::command(async)]
fn task_from_session(session_id: String) -> Result<serde_json::Value, String> {
    use hark_core::ports::SessionStore;
    let config = Config::load();
    let mut store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let _ = hark_plugin_claude::history::refresh_index(&config.projects_dir, &mut store);
    let summary = session_summary(&store, &session_id)
        .ok_or_else(|| format!("sessão {session_id} não está no índice"))?;
    let title = session_label(&summary);
    let tasks = store.board().map_err(|e| e.to_string())?;
    let updates = [hark_core::domain::board::BoardUpdate {
        titulo: title.clone(),
        status: hark_core::domain::board::TaskStatus::Doing,
        nota: Some("sessão recuperada".into()),
        sessao: None,
    }];
    let merged = hark_core::domain::board::apply_updates(
        tasks,
        &updates,
        &now_iso(),
        summary.cwd.as_deref(),
        Some(&session_id),
    );
    store.save_board(&merged).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "title": title, "workspace": summary.cwd, "session_id": session_id,
    }))
}

#[tauri::command]
fn board_move(title: String, status: hark_core::domain::board::TaskStatus) -> Result<(), String> {
    use hark_core::ports::SessionStore;
    let config = Config::load();
    let mut store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let tasks = store.board().map_err(|e| e.to_string())?;
    let tasks = hark_core::domain::board::set_status(tasks, &title, status, &now_iso());
    store.save_board(&tasks).map_err(|e| e.to_string())
}

fn with_board<T>(
    f: impl FnOnce(&mut SqliteStore, Vec<hark_core::domain::board::Task>) -> anyhow::Result<T>,
) -> Result<T, String> {
    use hark_core::ports::SessionStore;
    let config = Config::load();
    let mut store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let tasks = store.board().map_err(|e| e.to_string())?;
    f(&mut store, tasks).map_err(|e| e.to_string())
}

#[tauri::command]
fn board_rename(title: String, new_title: String) -> Result<(), String> {
    with_board(|store, tasks| {
        use hark_core::ports::SessionStore;
        let tasks = hark_core::domain::board::rename(tasks, &title, &new_title, &now_iso());
        store.save_board(&tasks)
    })
}

#[tauri::command]
fn board_pin(title: String) -> Result<(), String> {
    with_board(|store, tasks| {
        use hark_core::ports::SessionStore;
        let tasks = hark_core::domain::board::toggle_pin(tasks, &title);
        store.save_board(&tasks)
    })
}

#[derive(Serialize)]
struct GateOut {
    acao: hark_core::domain::gate::GateAction,
    confianca: f64,
    motivo: String,
    aviso: Option<String>,
    task_alvo: Option<String>,
    needs_confirmation: bool,
    cost_usd: Option<f64>,
}

/// One spoken sentence, classified into an action. Serialized flat so the
/// windows can switch on `kind` the way they already switch on plans.
#[derive(Serialize, Default)]
struct IntentOut {
    kind: String,
    project: Option<String>,
    session_id: Option<String>,
    session_title: Option<String>,
    instruction: Option<String>,
    question: Option<String>,
    options: Vec<IntentOption>,
    cost_usd: Option<f64>,
}

#[derive(Serialize)]
struct IntentOption {
    session_id: String,
    title: String,
    project: Option<String>,
}

/// The fallback that used to be prose.
///
/// When the zero-token grammar does not recognise a sentence, this asks
/// the LIGHT model to place it — with a catalog of the projects and
/// sessions that actually exist, plus the last lines of the conversation.
/// It answers with an action or an honest question, never a paragraph.
/// Cheaper than being wrong: a misrouted sentence used to cost a full
/// ask (~$0.04) and accomplish nothing.
#[tauri::command(async)]
fn classify_utterance(
    app: AppHandle,
    utterance: String,
    recent: Vec<String>,
) -> Result<IntentOut, String> {
    use hark_core::domain::voice_intent as vi;
    use hark_core::ports::SessionStore;

    let config = Config::load();
    let data_dir = config.data_dir();
    let store = SqliteStore::open(&data_dir.join("index.db")).map_err(|e| e.to_string())?;

    // Only sessions Hark can actually resume are offered as targets.
    let since = (Utc::now() - hark_core::chrono::Duration::days(30)).to_rfc3339();
    let sessions: Vec<vi::CatalogSession> = store
        .sessions_since(&since)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|s| {
            s.title.clone().map(|title| vi::CatalogSession {
                id: s.session_id,
                title,
                // The folder name is how a person refers to the project.
                project: s.cwd.as_deref().map(hark_core::domain::project::derive_name),
            })
        })
        .take(hark_core::domain::voice_intent::MAX_SESSIONS)
        .collect();

    // The focus comes from the SAME ledger the spoken plan uses — "nesse
    // chat" has to mean the window the person actually worked in last,
    // not whatever the calling window happened to remember.
    let focus = app
        .state::<voice::ActiveContext>()
        .0
        .lock()
        .unwrap()
        .current()
        .cloned()
        .unwrap_or_default();

    let catalog = vi::IntentCatalog {
        projects: state_file::load(&data_dir)
            .projects
            .iter()
            .map(|p| p.name.clone())
            .collect(),
        sessions,
        focused_project: focus.project_name,
        focused_session: focus.session_id,
        recent_exchange: recent,
    };

    // The free gate: plain questions and small talk never pay the model.
    if !vi::should_classify(&utterance, &catalog) {
        return Ok(IntentOut {
            kind: "question".into(),
            ..Default::default()
        });
    }

    let runner = hark_plugin_claude::cli::ClaudeCli {
        claude_bin: config.claude_bin_resolved(),
        work_dir: data_dir.clone(),
    };
    let prompt = vi::build_prompt(&utterance, &catalog);
    let request = hark_core::ports::TurnRequest {
        prompt: &prompt,
        images: &[],
        model: &config.models().light,
        system_prompt: vi::INTENT_SYSTEM_PROMPT,
        schema: vi::INTENT_SCHEMA,
        effort: "low",
    };
    let turn = runner.ask(&request, &mut |_| {}).map_err(|e| e.to_string())?;
    let decision = serde_json::from_str::<vi::IntentDecision>(&turn.raw).unwrap_or_default();
    let plan = vi::resolve(&decision, &catalog);

    record_live_spend(
        &config,
        hark_core::domain::spend::SpendKind::Gate,
        &hark_core::domain::spend::SpendMeta {
            label: Some("roteador de voz"),
            outcome: Some(&format!("intent:{}", decision.kind)),
            ..Default::default()
        },
        &turn,
    );

    let mut out = IntentOut {
        cost_usd: turn.cost_usd,
        ..Default::default()
    };
    match plan {
        vi::SpokenPlan::OpenProject { project, instruction } => {
            out.kind = "open_project".into();
            out.project = Some(project);
            out.instruction = instruction;
        }
        vi::SpokenPlan::OpenSession { session_id, title, instruction } => {
            out.kind = "open_session".into();
            out.session_id = Some(session_id);
            out.session_title = Some(title);
            out.instruction = instruction;
        }
        vi::SpokenPlan::Dispatch { session_id, instruction } => {
            out.kind = "dispatch".into();
            out.session_id = session_id;
            out.instruction = Some(instruction);
        }
        vi::SpokenPlan::Clarify { question, options } => {
            out.kind = "clarify".into();
            out.question = Some(question);
            out.options = options
                .into_iter()
                .map(|o| IntentOption {
                    session_id: o.id,
                    title: o.title,
                    project: o.project,
                })
                .collect();
        }
        vi::SpokenPlan::Status { session_id } => {
            out.kind = "status".into();
            out.session_id = session_id;
        }
        vi::SpokenPlan::Question => out.kind = "question".into(),
    }
    Ok(out)
}

/// Pure passthrough to the follow-up grammar: what did the user's reply
/// to "terminei lá — quer que eu faça algo?" actually mean.
#[tauri::command]
fn interpret_followup(utterance: String, label: String) -> Result<serde_json::Value, String> {
    use hark_core::domain::followup::{interpret, FollowUp};
    Ok(match interpret(&utterance, &label) {
        FollowUp::GoThere => serde_json::json!({ "kind": "go" }),
        FollowUp::StayHere => serde_json::json!({ "kind": "stay" }),
        FollowUp::DoThere(instruction) => {
            serde_json::json!({ "kind": "do", "instruction": instruction })
        }
        FollowUp::Unrelated => serde_json::json!({ "kind": "unrelated" }),
    })
}


/// The user's config as the settings UI sees it: current values + where
/// the file lives. The file itself stays the source of truth.
#[tauri::command]
fn config_read() -> Result<serde_json::Value, String> {
    let config = Config::load();
    let path = hark_core::config::config_path();
    Ok(serde_json::json!({
        "values": config,
        "path": path.display().to_string(),
        "claude_bin_resolved": config.claude_bin_resolved(),
        "whisper_model_resolved": config.whisper_model_path().display().to_string(),
        "data_dir": config.data_dir().display().to_string(),
    }))
}

/// The mother thread's persistent record of ask turns: newest `n` global
/// journal entries as structured Q&A — local file read, zero tokens.
#[tauri::command]
fn journal_recent(n: Option<usize>) -> Vec<hark_core::domain::memory::JournalTurn> {
    let config = Config::load();
    hark_core::adapters::memory_files::read_journal(&config.data_dir(), n.unwrap_or(12))
}

/// Write a flat {key: value} patch into config.toml, preserving comments
/// and unknown keys (hark_core::config::patch_toml). Hot-applies what it
/// can: a changed hotkey re-registers immediately; every window hears
/// config_changed and re-reads (theme, mode default, ceilings).
#[tauri::command]
fn config_write(app: AppHandle, patch: serde_json::Value) -> Result<(), String> {
    let path = hark_core::config::config_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let old_hotkey = Config::load().hotkey;
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let out = hark_core::config::patch_toml(&text, &patch).map_err(|e| e.to_string())?;
    std::fs::write(&path, out).map_err(|e| e.to_string())?;

    // Menu label follows the UI language without a restart.
    if patch.get("ui_language").is_some() {
        if let Ok(menu) = build_native_menu(&app) {
            let _ = app.set_menu(menu);
        }
    }
    if let Some(new_hotkey) = patch.get("hotkey").and_then(|v| v.as_str()) {
        if new_hotkey != old_hotkey {
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            let _ = app.global_shortcut().unregister(old_hotkey.as_str());
            app.global_shortcut()
                .register(new_hotkey)
                .map_err(|e| format!("atalho inválido: {e}"))?;
        }
    }
    let _ = app.emit("hark", serde_json::json!({ "kind": "config_changed" }));
    Ok(())
}

/// Voices the OS `say` engine offers, pt-* first (the answer voice).
#[tauri::command(async)]
fn tts_voices() -> Result<Vec<(String, String)>, String> {
    let out = std::process::Command::new("/usr/bin/say")
        .args(["-v", "?"])
        .output()
        .map_err(|e| e.to_string())?;
    let mut voices: Vec<(String, String)> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            // "Luciana             pt_BR    # Olá! ..."
            let lang_at = line.find('_').map(|i| i.saturating_sub(2))?;
            let name = line[..lang_at].trim();
            let lang = line[lang_at..].split_whitespace().next()?;
            (!name.is_empty()).then(|| (name.to_string(), lang.to_string()))
        })
        .collect();
    voices.sort_by_key(|(name, lang)| (!lang.starts_with("pt"), name.clone()));
    voices.dedup();
    Ok(voices)
}

/// Open a URL or local file with the OS (default browser/app) — NEVER
/// inside the webview: a click must not navigate the app away (that bug
/// turned a project window into the mother and lost the chat).
#[tauri::command]
fn open_external(target: String) -> Result<(), String> {
    let target = hark_core::config::expand_home(&target);
    std::process::Command::new("open")
        .arg(&target)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// What this machine knows about a session before dispatching to it.
pub(crate) fn session_facts(session_id: &str) -> hark_core::domain::precheck::SessionFacts {
    let mut facts = hark_core::domain::precheck::SessionFacts::default();
    let config = Config::load();
    let Ok(store) = SqliteStore::open(&config.data_dir().join("index.db")) else {
        return facts;
    };
    use hark_core::ports::SessionStore;
    if let Ok(Some(path)) = store.session_path(session_id) {
        if let Ok(meta) = std::fs::metadata(&path) {
            facts.size_mb = meta.len() as f64 / 1_048_576.0;
        }
    }
    if let Ok(Some(w)) = store.last_context_weight(session_id) {
        facts.context_pct = w
            .context_window
            .filter(|window| *window > 0)
            .map(|window| w.total as f64 / window as f64);
    }
    facts
}

/// Local, deterministic dispatch warnings (size, context weight) with
/// their actions — zero tokens, numbers always true. This replaces the
/// gate's LLM-authored cost warnings.
#[tauri::command(async)]
fn dispatch_prechecks(
    session_id: String,
) -> Result<Vec<hark_core::domain::precheck::Warning>, String> {
    Ok(hark_core::domain::precheck::prechecks(
        &session_facts(&session_id),
        Config::load().lang(),
    ))
}

/// The pre-execution evaluator: one cheap haiku call that decides where a
/// message goes BEFORE anything expensive runs. Runs with zero tools.
#[tauri::command(async)]
fn evaluate(
    message: String,
    focused_task: Option<String>,
    focused_session: Option<String>,
) -> Result<GateOut, String> {
    use hark_core::domain::gate;
    use hark_core::ports::SessionStore;

    let config = Config::load();
    let store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;

    // Real session title only: mismatch detection is the gate's ONE job.
    // Cost/size warnings are LOCAL now (dispatch_prechecks) — feeding the
    // size into the prompt is how "3.8MB" turned into a hallucinated
    // "MCP caiu" aviso on the confirm modal.
    let mut session_title = None;
    if let Some(sid) = &focused_session {
        if let Ok(Some(path)) = store.session_path(sid) {
            if let Ok(Some((summary, _, _))) = store.file_state(&path) {
                session_title = summary.title;
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
        focused_session_title: session_title,
        board_lines,
        live_workers: state_file::load(&config.data_dir())
            .workers
            .iter()
            .filter(|w| matches!(w.status, WorkerStatus::Running))
            .map(|w| w.summary.clone())
            .collect(),
    };

    // Same seam as the voice ask: the plugin's schema-constrained one-shot
    // runner — the gate stopped owning a hand-rolled spawn of the CLI.
    let runner = hark_plugin_claude::cli::ClaudeCli {
        claude_bin: config.claude_bin_resolved(),
        work_dir: config.data_dir(),
    };
    let prompt = gate::build_prompt(&message, &ctx);
    let request = hark_core::ports::TurnRequest {
        prompt: &prompt,
        images: &[],
        model: "haiku",
        system_prompt: gate::GATE_SYSTEM_PROMPT,
        schema: gate::GATE_SCHEMA,
        effort: "low",
    };
    let turn = runner.ask(&request, &mut |_| {}).map_err(|e| e.to_string())?;
    let decision_parse = serde_json::from_str::<hark_core::domain::gate::GateDecision>(&turn.raw);
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
        hark_core::domain::spend::SpendKind::Gate,
        &hark_core::domain::spend::SpendMeta {
            label: Some("avaliador"),
            session_id: ledger_session.as_deref(),
            outcome: Some(&outcome),
            ..Default::default()
        },
        &turn,
    );
    let mut decision = decision_parse
        .map_err(|e| format!("gate parse: {e}"))?
        .sanitized();
    // An aviso citing tools/MCPs or carrying numbers is fabricated by
    // construction (the gate receives neither) — drop it BEFORE it can
    // force a confirmation.
    decision.aviso = hark_core::domain::gate::credible_aviso(decision.aviso.take());
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

/// Slash commands accepted by each workspace's sessions, as announced by
/// the CLI's init event (key "" = the most recent list seen anywhere).
#[derive(Default)]
struct SlashRegistry(Mutex<HashMap<String, Vec<String>>>);

/// Universal built-ins shown before any worker has run in this workspace.
const SLASH_FALLBACK: &[&str] =
    &["compact", "context", "usage", "model", "rename", "clear", "review", "init"];

#[tauri::command]
fn slash_commands(
    state: State<'_, SlashRegistry>,
    workspace: Option<String>,
) -> Result<Vec<String>, String> {
    let map = state.inner().0.lock().unwrap();
    Ok(workspace
        .as_deref()
        .and_then(|w| map.get(w))
        .or_else(|| map.get(""))
        .cloned()
        .unwrap_or_else(|| SLASH_FALLBACK.iter().map(|s| s.to_string()).collect()))
}

/// Local board command spoken by the user ("mostra o log da X"). Resolves the
/// task by term overlap and performs the action; no LLM, no tokens.
/// Unregistered directories a spoken sentence names, as absolute paths.
/// Roots: the parents of every registered project, plus ~/Projects.
fn disk_project_matches(config: &Config, text: &str) -> Vec<String> {
    let registered = load_projects(config);
    let mut roots: Vec<std::path::PathBuf> = registered
        .iter()
        .filter_map(|p| std::path::Path::new(&p.path).parent().map(|d| d.to_path_buf()))
        .collect();
    if let Some(home) = dirs_home() {
        roots.push(home.join("Projects"));
    }
    roots.sort();
    roots.dedup();
    let mut names: Vec<(String, String)> = Vec::new(); // (dir name, abs path)
    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root) else { continue };
        for e in entries.flatten() {
            let path = e.path();
            if !path.is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || registered.iter().any(|p| p.path == path.to_string_lossy()) {
                continue;
            }
            names.push((name, path.to_string_lossy().to_string()));
        }
    }
    let dirs: Vec<&str> = names.iter().map(|(n, _)| n.as_str()).collect();
    hark_core::domain::project::dirs_matching_speech(text, &dirs)
        .into_iter()
        .filter_map(|hit| names.iter().find(|(n, _)| *n == hit).map(|(_, p)| p.clone()))
        .take(4)
        .collect()
}

fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

#[tauri::command]
fn task_command(
    text: String,
    focused: Option<String>,
) -> Result<Option<serde_json::Value>, String> {
    use hark_core::domain::task_command::TaskCommand;
    let Some(command) = hark_core::domain::task_command::parse(&text) else {
        // Fallback: "abre <nome>" without the word "projeto" ("ok, então
        // abra workspace fábrica"). Only fires when a REGISTERED project name
        // appears in the sentence — never guesses.
        let lower = text.to_lowercase();
        if ["abre", "abra", "abrir"].iter().any(|v| lower.contains(v)) {
            let config = Config::load();
            let projects = load_projects(&config);
            if let Some(hit) = hark_core::domain::project::find_spoken(&projects, &text) {
                return Ok(Some(serde_json::json!({
                    "kind": "open_project", "title": hit.name, "path": hit.path,
                    "instruction": null,
                })));
            }
            // The Intel 25/08 dead end: nothing registered matched, and the
            // answer was "cadastre o path na mão" — with the directory
            // sitting on disk. Look where projects actually live (the
            // parents of registered ones, ~/Projects as the default) and
            // OFFER what is there. One hit registers and opens; several
            // become a visible choice; zero falls through to the funnel.
            let matches = disk_project_matches(&config, &text);
            match matches.len() {
                0 => {}
                1 => {
                    let path = matches[0].clone();
                    return Ok(Some(match project_add(path) {
                        Ok(entry) => serde_json::json!({
                            "kind": "open_project", "title": entry.name,
                            "path": entry.path, "instruction": null,
                        }),
                        Err(message) => {
                            serde_json::json!({ "kind": "project_error", "title": message })
                        }
                    }));
                }
                _ => {
                    return Ok(Some(serde_json::json!({
                        "kind": "project_offer",
                        "query": text,
                        "candidates": matches,
                    })));
                }
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
                match hark_core::domain::project::find(&projects, project) {
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
            let hit = hark_core::domain::project::find(&projects, query)
                .or_else(|| hark_core::domain::project::find_spoken(&projects, query));
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
        // Both resolve on the window that owns the focused chat: compaction
        // is "/compact" sent to its session, mode goes to worker_set_mode.
        TaskCommand::Compact => {
            return Ok(Some(serde_json::json!({ "kind": "compact" })));
        }
        TaskCommand::SetMode { mode } => {
            return Ok(Some(serde_json::json!({ "kind": "set_mode", "mode": mode })));
        }
        // App-level settings live on the mother window.
        TaskCommand::OpenSettings => {
            return Ok(Some(serde_json::json!({ "kind": "open_settings" })));
        }
        TaskCommand::FindSession { query } => {
            // Every session on this machine is already indexed: recovering
            // one is a local lookup, never an agent digging through logs.
            let hits = session_hits(query, 6)?;
            return Ok(Some(serde_json::json!({
                "kind": "session_candidates", "query": query, "candidates": hits,
            })));
        }
        _ => {}
    }
    // Open/Switch are TARGETING: ambiguity becomes candidates on screen,
    // and a query with no board card falls back to the session index —
    // never a silent best-guess (the 19/08 incident class).
    if let TaskCommand::Open(q) | TaskCommand::Switch { query: q, .. } = &command {
        use hark_core::domain::matching::Match;
        let ranked =
            with_board(|_, tasks| Ok(hark_core::domain::board::find_ranked(&tasks, q)))?;
        return Ok(Some(match ranked {
            Match::Hit(task) => match &command {
                TaskCommand::Open(_) => serde_json::json!({
                    "kind": "open", "title": task.title,
                    "session_id": task.session_ids.last(),
                }),
                _ => {
                    let instruction = match &command {
                        TaskCommand::Switch { instruction, .. } => instruction.clone(),
                        _ => None,
                    };
                    serde_json::json!({
                        "kind": "switch", "title": task.title,
                        "session_id": task.session_ids.last(),
                        "note": task.note,
                        "instruction": instruction,
                    })
                }
            },
            Match::Ambiguous(tasks) => serde_json::json!({
                "kind": "task_candidates", "query": q,
                "candidates": tasks.iter().map(|t| serde_json::json!({
                    "title": t.title,
                    "session_id": t.session_ids.last(),
                    "workspace": t.workspace,
                })).collect::<Vec<_>>(),
            }),
            Match::None => {
                let hits = session_hits(q, 6)?;
                if hits.is_empty() {
                    serde_json::json!({ "kind": "not_found", "query": q })
                } else {
                    serde_json::json!({
                        "kind": "session_candidates", "query": q, "candidates": hits,
                    })
                }
            }
        }));
    }

    let query = match &command {
        TaskCommand::Pin(q) | TaskCommand::Archive(q) => q,
        // "renomeia (esse chat) para X": an empty query means whatever the
        // window has focused right now.
        TaskCommand::Rename { query, .. } if query.is_empty() => {
            match focused.as_deref().filter(|f| !f.is_empty()) {
                Some(f) => f,
                None => {
                    return Ok(Some(serde_json::json!({
                        "kind": "not_found",
                        "query": "nenhuma task focada pra renomear",
                    })))
                }
            }
        }
        TaskCommand::Rename { query, .. } => query,
        TaskCommand::Open(_)
        | TaskCommand::Switch { .. }
        | TaskCommand::OpenFile { .. }
        | TaskCommand::AddProject { .. }
        | TaskCommand::NewChat { .. }
        | TaskCommand::OpenProject { .. }
        | TaskCommand::OpenHq { .. }
        | TaskCommand::FindSession { .. }
        | TaskCommand::Compact
        | TaskCommand::SetMode { .. }
        | TaskCommand::OpenSettings => {
            unreachable!("handled above")
        }
    };
    with_board(|store, tasks| {
        use hark_core::ports::SessionStore;
        let Some(task) = hark_core::domain::board::find(&tasks, query) else {
            return Ok(Some(serde_json::json!({ "kind": "not_found", "query": query })));
        };
        let title = task.title.clone();
        match &command {
            TaskCommand::Rename { title: new, .. } => {
                let tasks = hark_core::domain::board::rename(tasks, &title, new, &now_iso());
                store.save_board(&tasks)?;
                Ok(Some(serde_json::json!({ "kind": "renamed", "title": new })))
            }
            TaskCommand::Pin(_) => {
                let tasks = hark_core::domain::board::toggle_pin(tasks, &title);
                store.save_board(&tasks)?;
                Ok(Some(serde_json::json!({ "kind": "pinned", "title": title })))
            }
            TaskCommand::Archive(_) => {
                let tasks = hark_core::domain::board::archive(tasks, &title);
                store.save_board(&tasks)?;
                Ok(Some(serde_json::json!({ "kind": "archived", "title": title })))
            }
            TaskCommand::Open(_)
            | TaskCommand::Switch { .. }
            | TaskCommand::OpenFile { .. }
            | TaskCommand::AddProject { .. }
            | TaskCommand::NewChat { .. }
            | TaskCommand::OpenProject { .. }
            | TaskCommand::OpenHq { .. }
            | TaskCommand::FindSession { .. }
            | TaskCommand::Compact
            | TaskCommand::SetMode { .. }
            | TaskCommand::OpenSettings => unreachable!("handled above"),
        }
    })
}

#[tauri::command]
fn board_archive(title: String) -> Result<(), String> {
    use hark_core::ports::SessionStore;
    let config = Config::load();
    let mut store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    let tasks = store.board().map_err(|e| e.to_string())?;
    let tasks = hark_core::domain::board::archive(tasks, &title);
    store.save_board(&tasks).map_err(|e| e.to_string())
}

/// Open (or focus) the dedicated window of one project — the VSCode-style
/// "one project, one window" model. The main window stays the orchestrator.
///
/// With `task`, that task lands focused in the chat of the window (clicking
/// a card on the global board goes straight back to work). A window that
/// already exists gets an event; a fresh one carries it in the URL, since
/// its webview is not listening yet.
#[tauri::command]
fn open_project_window(
    app: AppHandle,
    name: String,
    path: String,
    task: Option<String>,
    session: Option<String>,
) -> Result<(), String> {
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
        let _ = existing.unminimize();
        let _ = existing.set_focus();
        if let Some(title) = task {
            let _ = app.emit_to(
                label.as_str(),
                "hark",
                serde_json::json!({
                    "kind": "focus_task",
                    "title": title,
                    "session_id": session,
                }),
            );
        }
        return Ok(());
    }
    let mut url = format!(
        "index.html?project={}&name={}",
        urlencoding::encode(&path),
        urlencoding::encode(&name)
    );
    if let Some(title) = &task {
        url.push_str(&format!("&task={}", urlencoding::encode(title)));
    }
    if let Some(session) = &session {
        url.push_str(&format!("&session={}", urlencoding::encode(session)));
    }
    tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::App(url.into()))
        .title(format!("Hark — {name}"))
        .inner_size(1280.0, 820.0)
        // wry's native drop target swallows DOM dragover/drop; without this
        // the board's HTML5 card drag never lands (we take no file drops).
        .disable_drag_drop_handler()
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn bridge_paths(config: &Config) -> hark_plugin_claude::bridge::BridgePaths {
    let home = std::path::PathBuf::from(hark_core::config::expand_home("~"));
    let paths = hark_plugin_claude::bridge::BridgePaths::new(&home, &config.data_dir());
    // Pre-split installs kept the wrapper in the data dir: adopt them into
    // ~/.claude (script + backup move, settings repointed). No-op after.
    hark_plugin_claude::bridge::adopt_legacy(&paths, &config.data_dir());
    paths
}

/// The subscription windows (5h / weekly / per-model), the only numbers the
/// CLI keeps to itself — they exist solely in the statusLine payload, so
/// this returns None until the user installs the bridge.
#[tauri::command(async)]
fn subscription_limits() -> Result<Option<hark_plugin_claude::statusline::StatusLine>, String> {
    let config = Config::load();
    // 10 minutes: a status line renders on every turn, so anything older
    // means the user stopped working — showing it as current would lie.
    Ok(hark_plugin_claude::bridge::read(
        &bridge_paths(&config),
        600,
    ))
}

/// The /usage card, local and zero tokens: this session's per-model
/// breakdown from the ledger, the machine's last 24h, and the
/// subscription windows (when the bridge is installed). Every number is
/// computed in domain::usage; this only fetches and assembles.
#[tauri::command(async)]
fn usage_report(session_id: Option<String>) -> Result<serde_json::Value, String> {
    use hark_core::domain::usage;
    use hark_core::ports::{SpendGroup, SpendLedger, SpendQuery};
    let config = Config::load();
    let mut store =
        SqliteStore::open(&config.data_dir().join("index.db")).map_err(|e| e.to_string())?;
    // The machine table reads jsonl-backfilled rows: bring them current
    // first (incremental by byte offset — milliseconds when quiet).
    let _ = hark_plugin_claude::history::refresh_index(&config.projects_dir, &mut store);

    let session = session_id.and_then(|sid| {
        let rows = store.spend_rows_session(&sid).ok()?;
        // The context gauge answers "how full is THIS chat" even before
        // any live turn was ledgered (history-only sessions).
        let weight = store.last_context_weight(&sid).ok().flatten().unwrap_or_default();
        let pct = weight
            .context_window
            .filter(|c| *c > 0)
            .map(|c| (weight.total as f64 / c as f64).min(1.0));
        if rows.is_empty() && weight.total == 0 {
            return None;
        }
        let models = usage::breakdown(&rows);
        let cost: f64 = models.iter().map(|l| l.cost_usd).sum();
        let duration_ms: u64 = rows.iter().filter_map(|r| r.duration_ms).sum();
        Some(serde_json::json!({
            "models": models,
            "turns": rows.len(),
            "cost_usd": cost,
            "duration_ms": duration_ms,
            "cache_hit": usage::cache_hit(&models),
            "context": {
                "total": weight.total,
                "window": weight.context_window,
                "pct": pct,
            },
        }))
    });

    let day_iso = (Utc::now() - hark_core::chrono::Duration::hours(24)).to_rfc3339();
    let day = |group: SpendGroup| -> Vec<SpendAggOut> {
        store
            .spend_summary(&SpendQuery {
                since: Some(day_iso.clone()),
                group,
                source: hark_core::domain::spend::SpendSource::Live,
                workspace: None,
            })
            .unwrap_or_default()
            .into_iter()
            .map(SpendAggOut::from)
            .collect()
    };
    let kinds = day(SpendGroup::Kind);
    let mut labels = day(SpendGroup::Label);
    labels.retain(|a| a.cost_usd > 0.0);
    labels.truncate(5);
    let week_iso = (Utc::now() - hark_core::chrono::Duration::days(7)).to_rfc3339();
    let week_usd: f64 = store
        .spend_summary(&SpendQuery {
            since: Some(week_iso),
            group: SpendGroup::Kind,
            source: hark_core::domain::spend::SpendSource::Live,
            workspace: None,
        })
        .unwrap_or_default()
        .iter()
        .map(|a| a.cost_usd)
        .sum();

    // The whole MACHINE, exact: the jsonl index covers every agent session
    // on this box (terminal Claude Code included), tokens per model — the
    // official panel only approximates this.
    let machine_rows: Vec<_> = store
        .spend_rows(&day_iso, 20_000)
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.source == hark_core::domain::spend::SpendSource::Jsonl)
        .collect();
    let machine_models = usage::breakdown(&machine_rows);
    let mut machine_ws = store
        .spend_summary(&SpendQuery {
            since: Some(day_iso.clone()),
            group: SpendGroup::Workspace,
            source: hark_core::domain::spend::SpendSource::Jsonl,
            workspace: None,
        })
        .unwrap_or_default()
        .into_iter()
        .map(SpendAggOut::from)
        .collect::<Vec<_>>();
    machine_ws.sort_by(|a, b| {
        (b.input + b.output + b.cache_read).cmp(&(a.input + a.output + a.cache_read))
    });
    machine_ws.truncate(5);

    Ok(serde_json::json!({
        "session": session,
        "day": {
            "total_usd": kinds.iter().map(|a| a.cost_usd).sum::<f64>(),
            "week_usd": week_usd,
            "kinds": kinds,
            "top": labels,
        },
        "machine": {
            "models": machine_models,
            "turns": machine_rows.len(),
            "workspaces": machine_ws,
        },
        "limits": hark_plugin_claude::bridge::read(&bridge_paths(&config), 600),
    }))
}

#[tauri::command(async)]
fn statusline_bridge_status(
) -> Result<hark_plugin_claude::bridge::BridgeStatus, String> {
    let config = Config::load();
    Ok(hark_plugin_claude::bridge::status(&bridge_paths(
        &config,
    )))
}

/// Install the bridge. ONLY ever called from an explicit click: it edits
/// `~/.claude/settings.json` (after backing it up) and keeps whatever
/// status line was configured before running underneath.
#[tauri::command(async)]
fn statusline_bridge_install() -> Result<Option<String>, String> {
    let config = Config::load();
    hark_plugin_claude::bridge::install(&bridge_paths(&config))
        .map_err(|e| e.to_string())
}

#[tauri::command(async)]
fn statusline_bridge_uninstall() -> Result<(), String> {
    let config = Config::load();
    hark_plugin_claude::bridge::uninstall(&bridge_paths(&config))
        .map_err(|e| e.to_string())
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
        let _ = app.emit_to("main", "hark", serde_json::json!({ "kind": "main_tab", "tab": tab }));
    }
    Ok(())
}

/// Answer a pending permission request (live worker first, one-shot channel
/// as fallback).
#[tauri::command]
fn approve(
    app: AppHandle,
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
    // Every window shows this card: broadcast the outcome so they ALL
    // resolve, whoever answered (click, voice, the HUD).
    app.state::<PermLog>()
        .0
        .lock()
        .unwrap()
        .retain(|(id, _, _)| id != &request_id);
    let _ = app.emit(
        "hark",
        serde_json::json!({
            "kind": "permission_decided",
            "request_id": request_id,
            "allow": allow,
        }),
    );
    if let Some(task_id) = perms.0.lock().unwrap().remove(&request_id) {
        if let Some(worker) = live.0.lock().unwrap().get(&task_id) {
            let _ = worker.session.respond_permission(&request_id, decision);
        }
        return;
    }
    if let Some(tx) = pending.0.lock().unwrap().remove(&request_id) {
        let _ = tx.send(decision);
    }
}

/// Native macOS menu: the standard set plus "Settings…" (Cmd+,) under the
/// app's own submenu. Rebuilt on ui_language change (config_write) so the
/// label follows the UI language without a restart.
fn build_native_menu(
    handle: &tauri::AppHandle,
) -> Result<tauri::menu::Menu<tauri::Wry>, tauri::Error> {
    use tauri::menu::{AboutMetadata, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
    let label = if Config::load().ui_language == "en" {
        "Settings…"
    } else {
        "Configurações…"
    };
    let settings = MenuItemBuilder::with_id("settings", label)
        .accelerator("Cmd+,")
        .build(handle)?;
    let app_menu = SubmenuBuilder::new(handle, "hark")
        .about(Some(AboutMetadata::default()))
        .separator()
        .item(&settings)
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;
    // Clipboard/undo only work through these predefined items.
    let edit = SubmenuBuilder::new(handle, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let window = SubmenuBuilder::new(handle, "Window")
        .minimize()
        .separator()
        .close_window()
        .build()?;
    MenuBuilder::new(handle)
        .items(&[&app_menu, &edit, &window])
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Native macOS menu: the standard set plus "Settings…" (Cmd+,)
        // under the app's own submenu — it fronts the MOTHER window with
        // the settings modal open (machine config lives there).
        .menu(build_native_menu)
        .on_menu_event(|app, event| {
            if event.id() == "settings" {
                let _ = focus_main(app.clone(), Some("settings".into()));
            }
        })
        .manage(terminal::Terminals::default())
        .manage(voice::ActiveContext::default())
        .manage(SlashRegistry::default())
        .manage(Pending(Mutex::new(HashMap::new())))
        .manage(PermLog::default())
        .manage(BatchState::default())
        .manage(MicLease::default())
        .manage(LiveWorkers(Mutex::new(HashMap::new())))
        .manage(WorkerPermissions(Mutex::new(HashMap::new())))
        .setup(|app| {
            // Move data written under the old product name (vox) into the
            // hark dirs BEFORE anything reads config or touches the data dir.
            hark_core::config::migrate_legacy();
            // The chat's soul file exists from the first boot (settings can
            // open it before the chat ever spawns).
            ensure_persona(&Config::load());
            // Warm whisper so the first mic use is instant. Fire and
            // forget: the mic path reports "loading" instead of waiting.
            ensure_speech_loading();
            // Register the global mic hotkey (config `hotkey`).
            {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                let hotkey = Config::load().hotkey;
                if let Err(err) = app.handle().global_shortcut().register(hotkey.as_str()) {
                    eprintln!("hark: hotkey global '{hotkey}' não registrada: {err}");
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            overview,
            use_context,
            journal_recent,
            route_text,
            speak,
            speak_stop,
            hear_once,
            hear_stop,
            hear_abort,
            ask_text,
            dispatch_text,
            worker_start,
            worker_send,
            worker_restart_light,
            savings_summary,
            eco_status,
            setup_status,
            agent_plugins,
            agent_plugin_select,
            setup_download_model,
            setup_mark_done,
            board_subtask_toggle,
            ask_lane,
            hark_chat_send,
            hark_chat_status,
            worker_set_mode,
            worker_set_model,
            worker_stop,
            chat_start,
            project_add,
            project_remove,
            project_files,
            file_read,
            file_save,
            read_transcript,
            session_owners,
            repo_states,
            crossref_context,
            transcript_since,
            find_session,
            session_stats,
            spend_summary,
            spend_top_sessions,
            session_context_weight,
            task_command,
            slash_commands,
            interpret_verdict,
            dispatch_prechecks,
            classify_utterance,
            interpret_followup,
            config_read,
            config_write,
            tts_voices,
            open_external,
            evaluate,
            board_move,
            board_rename,
            board_pin,
            board_archive,
            voice::set_active_context,
            voice::plan_utterance,
            voice::voice_execute,
            voice::hud_show,
            voice::hud_hide,
            terminal::term_open,
            terminal::term_write,
            terminal::term_resize,
            terminal::term_close,
            session_candidates,
            project_sessions,
            task_from_session,
            open_project_window,
            focus_main,
            subscription_limits,
            usage_report,
            statusline_bridge_status,
            statusline_bridge_install,
            statusline_bridge_uninstall,
            approve
        ])
        // Self-update: check() hits the public releases repo's latest.json,
        // the minisign signature is verified against the pubkey baked into
        // tauri.conf.json, and process::relaunch is the "restart now" click.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // Global hotkey (default cmd+shift+space, config `hotkey`): from
        // ANY app, the voice button.
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        // Voice is global: the hotkey opens the floating HUD
                        // over whatever is on screen, never a specific window.
                        voice::show_hud(app);
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
            // Focus ledger: the OS tells us which PROJECT window the user
            // is working in; the spoken word follows it. Mother/HUD focus
            // is a no-op inside the ledger, closing falls back.
            let label = window.label().to_string();
            if label.starts_with("proj-") {
                let app = window.app_handle();
                let ctx = app.state::<voice::ActiveContext>();
                match event {
                    tauri::WindowEvent::Focused(true) => {
                        ctx.0.lock().unwrap().focused(&label);
                    }
                    tauri::WindowEvent::Destroyed => {
                        ctx.0.lock().unwrap().destroyed(&label);
                    }
                    _ => {}
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running hark");
}

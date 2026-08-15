//! Headless driver: `vox ask "..."`, `vox index`, `vox sessions`.

use vox_core::adapters::claude_cli::ClaudeCli;
use vox_core::adapters::git_collect::GitCli;
use vox_core::adapters::jsonl_scan::refresh_index;
use vox_core::adapters::live_sessions::ClaudeAgentsCli;
use vox_core::adapters::sqlite_store::SqliteStore;
use vox_core::adapters::state_file;
use vox_core::app::ask::{ask, build_snapshot, AskDeps};
use vox_core::config::Config;
use vox_core::domain::claude_event::ClaudeEvent;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.split_first() {
        Some((cmd, rest)) if cmd == "ask" && !rest.is_empty() => cmd_ask(&rest.join(" ")),
        Some((cmd, rest)) if cmd == "prompt" && !rest.is_empty() => cmd_prompt(&rest.join(" ")),
        Some((cmd, _)) if cmd == "index" => cmd_index(),
        Some((cmd, _)) if cmd == "sessions" => cmd_sessions(),
        Some((cmd, rest)) if cmd == "use" && rest.len() == 1 => cmd_use(&rest[0]),
        Some((cmd, _)) if cmd == "contexts" => cmd_contexts(),
        Some((cmd, rest)) if cmd == "dispatch" && !rest.is_empty() => {
            let (session, words) = match rest.split_first() {
                Some((flag, tail)) if flag == "--session" && tail.len() >= 2 => {
                    (Some(tail[0].clone()), tail[1..].join(" "))
                }
                _ => (None, rest.join(" ")),
            };
            cmd_dispatch(&words, session.as_deref())
        }
        Some((cmd, _)) if cmd == "ps" => cmd_ps(),
        Some((cmd, _)) if cmd == "setup" => cmd_setup(),
        Some((cmd, _)) if cmd == "hear" => cmd_hear(),
        Some((cmd, _)) if cmd == "listen" => cmd_listen(),
        _ => {
            eprintln!(
                "usage: vox listen | vox hear | vox setup | vox ask \"<q>\" | vox dispatch [--session <id>] \"<instruction>\" | vox ps | vox index | vox sessions | vox use <context> | vox contexts"
            );
            2
        }
    };
    std::process::exit(code);
}

fn cmd_use(name: &str) -> i32 {
    let config = Config::load();
    let known = name == "all" || config.context(name).is_some();
    if !known {
        eprintln!(
            "vox: unknown context '{name}'. Declared: all, {}",
            config.context_names().join(", ")
        );
        return 1;
    }
    // Preserve the worker registry; `use` only changes the focus.
    let state = state_file::GlobalState {
        active_context: (name != "all").then(|| name.to_string()),
        ..state_file::load(&config.data_dir())
    };
    match state_file::save(&config.data_dir(), &state) {
        Ok(()) => {
            println!("context: {name}");
            0
        }
        Err(err) => {
            eprintln!("vox: {err:#}");
            1
        }
    }
}

fn cmd_contexts() -> i32 {
    let config = Config::load();
    let active = state_file::load(&config.data_dir())
        .active_context
        .unwrap_or_else(|| config.default_context.clone());
    for name in std::iter::once("all".to_string()).chain(config.context_names()) {
        let marker = if name == active { "*" } else { " " };
        println!("{marker} {name}");
    }
    0
}

fn open_store(config: &Config) -> anyhow::Result<SqliteStore> {
    SqliteStore::open(&config.data_dir().join("index.db"))
}

fn cmd_ask(question: &str) -> i32 {
    let config = Config::load();
    let result = (|| -> anyhow::Result<_> {
        let mut store = open_store(&config)?;
        let state = state_file::load(&config.data_dir());
        let mut deps = AskDeps {
            active_context: state.active_context,
            workers: state.workers,
            journal: &vox_core::adapters::memory_files::VoxDir,
            store: &mut store,
            live: &ClaudeAgentsCli {
                claude_bin: config.claude_bin_resolved(),
            },
            repos: &GitCli,
            runner: &ClaudeCli {
                claude_bin: config.claude_bin_resolved(),
                model: config.model.clone(),
                work_dir: config.data_dir(),
            },
            config: &config,
        };
        ask(question, &mut deps, &mut |event| {
            // StructuredOutput is the schema mechanism, not a real tool.
            if let ClaudeEvent::ToolUse { name, .. } = event {
                if name != "StructuredOutput" {
                    eprintln!("… running {name}");
                }
            }
        })
    })();

    match result {
        Ok(turn) if !turn.is_error => {
            match turn.reply {
                Some(reply) => {
                    println!("🔊 {}\n", reply.fala);
                    println!("{}", reply.detalhes);
                    for item in &reply.itens {
                        println!("  • {item}");
                    }
                }
                None => println!("{}", turn.raw),
            }
            if let Some(cost) = turn.cost_usd {
                eprintln!("\n[cost ${cost:.4}]");
            }
            0
        }
        Ok(turn) => {
            eprintln!("claude error: {}", turn.raw);
            1
        }
        Err(err) => {
            eprintln!("vox: {err:#}");
            1
        }
    }
}

/// Debug helper: print the exact prompt `ask` would send.
fn cmd_prompt(question: &str) -> i32 {
    let config = Config::load();
    let result = (|| -> anyhow::Result<_> {
        let mut store = open_store(&config)?;
        let state = state_file::load(&config.data_dir());
        let mut deps = AskDeps {
            active_context: state.active_context,
            workers: state.workers,
            journal: &vox_core::adapters::memory_files::VoxDir,
            store: &mut store,
            live: &ClaudeAgentsCli {
                claude_bin: config.claude_bin_resolved(),
            },
            repos: &GitCli,
            runner: &NoopRunner,
            config: &config,
        };
        let snapshot = vox_core::app::ask::snapshot_for_question(&mut deps, question)?;
        Ok(vox_core::domain::prompt::build(question, &snapshot))
    })();
    match result {
        Ok(prompt) => {
            println!("{prompt}");
            0
        }
        Err(err) => {
            eprintln!("vox: {err:#}");
            1
        }
    }
}

fn cmd_index() -> i32 {
    let config = Config::load();
    match open_store(&config)
        .and_then(|mut store| refresh_index(&config.projects_dir, &mut store))
    {
        Ok(n) => {
            println!("indexed {n} changed file(s)");
            0
        }
        Err(err) => {
            eprintln!("vox: {err:#}");
            1
        }
    }
}

fn cmd_sessions() -> i32 {
    let config = Config::load();
    let result = (|| -> anyhow::Result<_> {
        let mut store = open_store(&config)?;
        let state = state_file::load(&config.data_dir());
        let mut deps = AskDeps {
            active_context: state.active_context,
            workers: state.workers,
            journal: &vox_core::adapters::memory_files::VoxDir,
            store: &mut store,
            live: &ClaudeAgentsCli {
                claude_bin: config.claude_bin_resolved(),
            },
            repos: &GitCli,
            runner: &NoopRunner,
            config: &config,
        };
        build_snapshot(&mut deps)
    })();

    match result {
        Ok(snapshot) => {
            for s in &snapshot.sessions {
                println!(
                    "{}  {}  [{}]  {}",
                    s.last_ts.as_deref().unwrap_or("?"),
                    s.session_id,
                    s.git_branch.as_deref().unwrap_or("?"),
                    s.title
                        .as_deref()
                        .or(s.last_prompt.as_deref())
                        .unwrap_or("(empty)"),
                );
            }
            eprintln!(
                "\n{} session(s), {} live",
                snapshot.sessions.len(),
                snapshot.live.len()
            );
            0
        }
        Err(err) => {
            eprintln!("vox: {err:#}");
            1
        }
    }
}

fn cmd_dispatch(instruction: &str, session_override: Option<&str>) -> i32 {
    use vox_core::adapters::{memory_files, worker};
    use vox_core::app::dispatch::{plan, Plan, Planned};
    use vox_core::domain::claude_event::PermissionDecision;
    use vox_core::domain::memory::{WorkerRecord, WorkerStatus};

    let config = Config::load();
    let planned = (|| -> anyhow::Result<Plan> {
        let mut store = open_store(&config)?;
        let state = state_file::load(&config.data_dir());
        let mut deps = AskDeps {
            active_context: state.active_context,
            workers: state.workers,
            journal: &vox_core::adapters::memory_files::VoxDir,
            store: &mut store,
            live: &ClaudeAgentsCli {
                claude_bin: config.claude_bin_resolved(),
            },
            repos: &GitCli,
            runner: &NoopRunner,
            config: &config,
        };
        plan(&mut deps, instruction, session_override)
    })();

    let planned: Planned = match planned {
        Ok(Plan::Ready(p)) => p,
        Ok(Plan::NeedsChoice(candidates)) => {
            eprintln!("Sessões candidatas, escolha uma com --session <id>:");
            for c in candidates {
                eprintln!(
                    "  {}  [{}]  {}",
                    c.session_id,
                    c.last_ts.as_deref().unwrap_or("?"),
                    c.title.as_deref().or(c.last_prompt.as_deref()).unwrap_or("?")
                );
            }
            return 3;
        }
        Ok(Plan::TargetBusy(s)) => {
            eprintln!(
                "Sessão {} está ABERTA em um terminal agora. Feche-a ou escolha outra com --session.",
                s.session_id
            );
            return 4;
        }
        Ok(Plan::NoMatch) => {
            eprintln!("Nenhuma sessão bate com essa instrução. Use --session <id> (veja vox sessions).");
            return 5;
        }
        Err(err) => {
            eprintln!("vox: {err:#}");
            return 1;
        }
    };

    let task_id = format!(
        "t-{}-{}",
        &planned.session.session_id[..6.min(planned.session.session_id.len())],
        chrono_compact()
    );
    println!(
        "▶ {task_id}: retomando sessão {} em {}",
        planned.session.session_id,
        planned.workspace_root.display()
    );

    let brief = format!(
        "# Vox dispatch {task_id}\n\n- session: {}\n- workspace: {}\n\n## Instruction\n\n{instruction}\n",
        planned.session.session_id,
        planned.workspace_root.display()
    );
    let _ = memory_files::write_brief(&planned.workspace_root, &task_id, &brief);

    // Register as running (machine-wide view).
    let mut state = state_file::load(&config.data_dir());
    state.workers.push(WorkerRecord {
        task_id: task_id.clone(),
        context: "".into(),
        workspace: planned.workspace_root.display().to_string(),
        session_id: planned.session.session_id.clone(),
        status: WorkerStatus::Running,
        started_at: now_iso(),
        summary: instruction.chars().take(120).collect(),
    });
    let _ = state_file::save(&config.data_dir(), &state);

    let spawn = worker::WorkerSpawn {
        claude_bin: config.claude_bin_resolved(),
        cwd: planned.workspace_root.clone(),
        session_id: planned.session.session_id.clone(),
        instruction: instruction.to_string(),
    };
    let result = worker::run(
        &spawn,
        &mut |running| eprintln!("  worker pid {}", running.pid),
        &mut |tool, input| {
            eprintln!("\n🔐 {tool} pede permissão:\n{input}");
            eprint!("aprovar? [y/N] ");
            let mut answer = String::new();
            let _ = std::io::stdin().read_line(&mut answer);
            if answer.trim().eq_ignore_ascii_case("y") {
                PermissionDecision::Allow
            } else {
                PermissionDecision::Deny
            }
        },
        &mut |event| {
            if let ClaudeEvent::ToolUse { name, input } = event {
                let short: String = input.chars().take(90).collect();
                eprintln!("  [{task_id}] {name} {short}");
            }
        },
    );

    // Close out: registry + workspace state + exit code.
    let (status, code, summary) = match &result {
        Ok(turn) if !turn.is_error => (WorkerStatus::Done, 0, turn.raw.clone()),
        Ok(turn) => (WorkerStatus::Failed, 1, turn.raw.clone()),
        Err(err) => (WorkerStatus::Failed, 1, format!("{err:#}")),
    };
    let mut state = state_file::load(&config.data_dir());
    if let Some(record) = state.workers.iter_mut().find(|w| w.task_id == task_id) {
        record.status = status;
        record.summary = summary.chars().take(200).collect();
    }
    let _ = state_file::save(&config.data_dir(), &state);
    let _ = memory_files::append_state(
        &planned.workspace_root,
        &format!("- {} {task_id} [{:?}] {instruction}", now_iso(), status),
    );

    match result {
        Ok(turn) => {
            println!("\n{}", turn.raw);
            if let Some(cost) = turn.cost_usd {
                eprintln!("[cost ${cost:.4}]");
            }
            code
        }
        Err(err) => {
            eprintln!("vox: {err:#}");
            code
        }
    }
}

fn cmd_ps() -> i32 {
    let config = Config::load();
    let state = state_file::load(&config.data_dir());
    if state.workers.is_empty() {
        println!("nenhum worker registrado");
        return 0;
    }
    for w in &state.workers {
        println!(
            "{}  {:?}  {}  sessão {}  {}",
            w.task_id, w.status, w.workspace, w.session_id, w.summary
        );
    }
    0
}

/// Download the default whisper model into the data dir.
fn cmd_setup() -> i32 {
    let config = Config::load();
    let path = config.whisper_model_path();
    if path.exists() {
        println!("whisper model ok: {}", path.display());
        return 0;
    }
    let dir = path.parent().expect("model dir");
    if std::fs::create_dir_all(dir).is_err() {
        eprintln!("vox: cannot create {}", dir.display());
        return 1;
    }
    let url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin";
    println!("downloading {url} (~466MB)…");
    let status = std::process::Command::new("curl")
        .args(["-fSL", "--progress-bar", "-o"])
        .arg(&path)
        .arg(url)
        .status();
    match status {
        Ok(s) if s.success() => {
            println!("done: {}", path.display());
            0
        }
        _ => {
            eprintln!("vox: download failed");
            1
        }
    }
}

fn load_voice(config: &Config) -> anyhow::Result<(vox_core::adapters::whisper_stt::WhisperStt, vox_core::adapters::say_tts::SayTts, vox_core::adapters::cpal_audio::CpalMic)> {
    use vox_core::adapters::{cpal_audio::CpalMic, say_tts::SayTts, whisper_stt::WhisperStt};
    eprint!("loading whisper… ");
    let stt = WhisperStt::load(&config.whisper_model_path(), &config.language, &config.vocab)?;
    stt.warmup();
    eprintln!("ready");
    Ok((stt, SayTts { voice: config.voice.clone() }, CpalMic::default()))
}

/// Mic test: record one utterance, print the transcript. No Claude, no cost.
fn cmd_hear() -> i32 {
    use vox_core::ports::{AudioIn, Cue, Stt, Tts};
    let config = Config::load();
    let (stt, tts, mic) = match load_voice(&config) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("vox: {err:#}");
            return 1;
        }
    };
    tts.beep(Cue::Listening);
    eprintln!("🎤 fala (corta sozinho no silêncio)…");
    match mic.record_utterance().and_then(|audio| {
        tts.beep(Cue::Captured);
        stt.transcribe(&audio)
    }) {
        Ok(text) => {
            println!("{text}");
            0
        }
        Err(err) => {
            tts.beep(Cue::Error);
            eprintln!("vox: {err:#}");
            1
        }
    }
}

/// The JARVIS loop: hear -> route (ask|dispatch) -> speak.
fn cmd_listen() -> i32 {
    use vox_core::domain::intent::{is_affirmative, route, Route};
    use vox_core::ports::{AudioIn, Cue, Stt, Tts};

    let config = Config::load();
    let (stt, tts, mic) = match load_voice(&config) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("vox: {err:#}");
            return 1;
        }
    };
    let hear = |prompt: &str| -> Option<String> {
        eprintln!("{prompt}");
        tts.beep(Cue::Listening);
        let audio = mic.record_utterance().ok()?;
        tts.beep(Cue::Captured);
        let text = stt.transcribe(&audio).ok()?;
        (!text.is_empty()).then_some(text)
    };

    eprintln!("vox ouvindo. Diga \"sair\" para encerrar. Ctrl+C também funciona.");
    loop {
        let Some(text) = hear("🎤 pode falar…") else {
            continue;
        };
        eprintln!("» {text}");
        let lower = text.to_lowercase();
        if ["sair", "encerra", "tchau", "desliga"].iter().any(|w| lower.contains(w)) {
            let _ = tts.speak("Até mais.");
            return 0;
        }
        match route(&text) {
            Route::Ask => {
                let code = cmd_ask_spoken(&text, &tts);
                if code != 0 {
                    tts.beep(Cue::Error);
                }
            }
            Route::Dispatch => {
                let _ = tts.speak(&format!("Entendi: {text}. Confirma?"));
                match hear("🎤 confirma? (sim/não)…") {
                    Some(answer) if is_affirmative(&answer) => {
                        let _ = tts.speak("Despachando. Aprovações continuam pelo teclado.");
                        let code = cmd_dispatch(&text, None);
                        let _ = match code {
                            0 => tts.speak("Tarefa concluída."),
                            3 => tts.speak("Ficou ambíguo. Olha o terminal e escolhe a sessão."),
                            4 => tts.speak("A sessão alvo está aberta num terminal. Fecha ela primeiro."),
                            5 => tts.speak("Não achei sessão pra isso. Usa o terminal com --session."),
                            _ => tts.speak("Falhou. Detalhes no terminal."),
                        };
                    }
                    _ => {
                        let _ = tts.speak("Cancelado.");
                    }
                }
            }
        }
    }
}

/// `ask` variant that also speaks the `fala` field.
fn cmd_ask_spoken(question: &str, tts: &impl vox_core::ports::Tts) -> i32 {
    let config = Config::load();
    let result = (|| -> anyhow::Result<_> {
        let mut store = open_store(&config)?;
        let state = state_file::load(&config.data_dir());
        let mut deps = AskDeps {
            active_context: state.active_context,
            workers: state.workers,
            journal: &vox_core::adapters::memory_files::VoxDir,
            store: &mut store,
            live: &ClaudeAgentsCli {
                claude_bin: config.claude_bin_resolved(),
            },
            repos: &GitCli,
            runner: &ClaudeCli {
                claude_bin: config.claude_bin_resolved(),
                model: config.model.clone(),
                work_dir: config.data_dir(),
            },
            config: &config,
        };
        ask(question, &mut deps, &mut |_| {})
    })();

    match result {
        Ok(turn) if !turn.is_error => {
            if let Some(reply) = turn.reply {
                println!("🔊 {}\n\n{}", reply.fala, reply.detalhes);
                for item in &reply.itens {
                    println!("  • {item}");
                }
                let _ = tts.speak(&reply.fala);
            } else {
                println!("{}", turn.raw);
                let _ = tts.speak(&turn.raw.chars().take(300).collect::<String>());
            }
            0
        }
        Ok(turn) => {
            eprintln!("claude error: {}", turn.raw);
            1
        }
        Err(err) => {
            eprintln!("vox: {err:#}");
            1
        }
    }
}

fn now_iso() -> String {
    use vox_core::chrono::{SecondsFormat, Utc};
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn chrono_compact() -> String {
    use vox_core::chrono::Utc;
    Utc::now().format("%m%d%H%M%S").to_string()
}

/// `sessions` never talks to Claude.
struct NoopRunner;
impl vox_core::ports::AgentRunner for NoopRunner {
    fn ask(
        &self,
        _prompt: &str,
        _image: Option<(&str, &str)>,
        _on_event: &mut dyn FnMut(&ClaudeEvent),
    ) -> anyhow::Result<vox_core::domain::claude_event::TurnResult> {
        anyhow::bail!("not used")
    }
}

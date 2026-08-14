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
        _ => {
            eprintln!(
                "usage: vox ask \"<q>\" | vox dispatch [--session <id>] \"<instruction>\" | vox ps | vox index | vox sessions | vox use <context> | vox contexts"
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
                claude_bin: config.claude_bin.clone(),
            },
            repos: &GitCli,
            runner: &ClaudeCli {
                claude_bin: config.claude_bin.clone(),
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
                claude_bin: config.claude_bin.clone(),
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
                claude_bin: config.claude_bin.clone(),
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
                claude_bin: config.claude_bin.clone(),
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
        claude_bin: config.claude_bin.clone(),
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
        _on_event: &mut dyn FnMut(&ClaudeEvent),
    ) -> anyhow::Result<vox_core::domain::claude_event::TurnResult> {
        anyhow::bail!("not used")
    }
}

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
        _ => {
            eprintln!(
                "usage: vox ask \"<question>\" | vox index | vox sessions | vox use <context> | vox contexts"
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
    let state = state_file::GlobalState {
        active_context: (name != "all").then(|| name.to_string()),
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
        let mut deps = AskDeps {
            active_context: state_file::load(&config.data_dir()).active_context,
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
        let mut deps = AskDeps {
            active_context: state_file::load(&config.data_dir()).active_context,
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
        let mut deps = AskDeps {
            active_context: state_file::load(&config.data_dir()).active_context,
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

//! Headless driver: `hark ask "..."`, `hark index`, `hark sessions`.
//!
//! `hark ask` resolves its agent the way the app does (`[agent] ask`, else
//! the selected default, else claude when it is here, else the first
//! usable backend) and speaks tiers through the same runner. History
//! (`index`, `sessions`, the live list) is still claude's files on disk
//! until F9.4 gives ACP sessions a recorded history; on a machine with no
//! claude those commands say so, never silently.

use hark_plugin_claude::cli::ClaudeCli;
use hark_core::adapters::git_collect::GitCli;
use hark_plugin_claude::history::refresh_index;
use hark_plugin_claude::live::ClaudeAgentsCli;
use hark_core::adapters::sqlite_store::SqliteStore;
use hark_core::adapters::state_file;
use hark_core::app::ask::{ask, build_snapshot, AskDeps};
use hark_core::config::Config;
use hark_plugin_claude::stream::ClaudeEvent;

fn main() {
    // Adopt data written under the old product name before anything reads it.
    hark_core::config::migrate_legacy();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.split_first() {
        Some((cmd, rest)) if cmd == "ask" && !rest.is_empty() => cmd_ask(&rest.join(" ")),
        Some((cmd, rest)) if cmd == "prompt" && !rest.is_empty() => cmd_prompt(&rest.join(" ")),
        Some((cmd, rest)) if cmd == "index" => {
            cmd_index(rest.first().is_some_and(|f| f == "--full"))
        }
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
        Some((cmd, rest)) if cmd == "ps" => cmd_ps(rest.first().is_some_and(|f| f == "--clear")),
        Some((cmd, rest)) if cmd == "spend" => cmd_spend(rest),
        Some((cmd, _)) if cmd == "board" => cmd_board(),
        Some((cmd, rest)) if cmd == "setup" => cmd_setup(rest),
        Some((cmd, _)) if cmd == "update" => cmd_update(),
        Some((cmd, _)) if cmd == "hear" => cmd_hear(),
        Some((cmd, _)) if cmd == "listen" => cmd_listen(),
        _ => {
            eprintln!(
                "usage: hark listen | hark hear | hark setup [--model small|large-v3-turbo] | hark update | hark ask \"<q>\" | hark dispatch [--session <id>] \"<instruction>\" | hark ps | hark spend [--day|--week|--project] [--rebuild] [--export csv|json] | hark index | hark sessions | hark use <context> | hark contexts"
            );
            2
        }
    };
    std::process::exit(code);
}

/// The spend ledger from the terminal: `hark spend [--day|--week|--project]
/// [--rebuild]`. USD comes from live rows; tokens from the jsonl history —
/// the two are never summed together.
fn cmd_spend(rest: &[String]) -> i32 {
    use hark_core::domain::spend::SpendSource;
    use hark_core::ports::{SpendGroup, SpendLedger, SpendQuery};
    let config = Config::load();
    let has = |flag: &str| rest.iter().any(|f| f == flag);
    let result = (|| -> anyhow::Result<i32> {
        let mut store = open_store(&config)?;
        if has("--rebuild") {
            let scanned =
                hark_plugin_claude::history::rebuild_spend(&config.projects_dir, &mut store)?;
            eprintln!("[rebuild: {scanned} arquivos de sessão varridos]");
        }
        let since = {
            use hark_core::chrono::{Duration, Utc};
            if has("--day") {
                Some((Utc::now() - Duration::hours(24)).to_rfc3339())
            } else if has("--week") {
                Some((Utc::now() - Duration::days(7)).to_rfc3339())
            } else {
                None
            }
        };
        // --export csv|json: raw rows to stdout (team-side aggregation).
        // USD stays live-only, tokens jsonl-only — the source column keeps
        // the never-sum rule enforceable downstream.
        if let Some(pos) = rest.iter().position(|f| f == "--export") {
            let format = rest.get(pos + 1).map(String::as_str).unwrap_or("csv");
            let rows = store.spend_rows(since.as_deref().unwrap_or("0"), 1_000_000)?;
            match format {
                "json" => println!("{}", serde_json::to_string_pretty(&rows)?),
                "csv" => {
                    let esc = |v: &str| {
                        if v.contains([',', '"', '\n']) {
                            format!("\"{}\"", v.replace('"', "\"\""))
                        } else {
                            v.to_string()
                        }
                    };
                    println!(
                        "ts,kind,source,task_id,label,session_id,workspace,model,input_tokens,output_tokens,cache_read_tokens,cache_created_tokens,cost_usd,duration_ms,is_error,is_sidechain,context_window,request_id,outcome"
                    );
                    for r in &rows {
                        println!(
                            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                            r.ts,
                            r.kind.as_str(),
                            r.source.as_str(),
                            esc(r.task_id.as_deref().unwrap_or("")),
                            esc(r.label.as_deref().unwrap_or("")),
                            esc(r.session_id.as_deref().unwrap_or("")),
                            esc(r.workspace.as_deref().unwrap_or("")),
                            esc(&r.model),
                            r.usage.input,
                            r.usage.output,
                            r.usage.cache_read,
                            r.usage.cache_created,
                            r.cost_usd.map(|v| v.to_string()).unwrap_or_default(),
                            r.duration_ms.map(|v| v.to_string()).unwrap_or_default(),
                            r.is_error as u8,
                            r.is_sidechain as u8,
                            r.context_window.map(|v| v.to_string()).unwrap_or_default(),
                            esc(r.request_id.as_deref().unwrap_or("")),
                            esc(r.outcome.as_deref().unwrap_or("")),
                        );
                    }
                }
                other => anyhow::bail!("formato de export desconhecido: {other} (csv|json)"),
            }
            eprintln!("[{} linhas exportadas]", rows.len());
            return Ok(0);
        }
        // --savings: the meter, same formulas as the UI card.
        if has("--savings") {
            let kinds = store.spend_summary(&SpendQuery {
                since: since.clone(),
                group: SpendGroup::Kind,
                source: SpendSource::Live,
                workspace: None,
            })?;
            let models = store.spend_summary(&SpendQuery {
                since: since.clone(),
                group: SpendGroup::Model,
                source: SpendSource::Live,
                workspace: None,
            })?;
            let gate_blocked = store
                .spend_rows(since.as_deref().unwrap_or("0"), 1_000_000)?
                .iter()
                .filter(|r| {
                    // "gate:meta_vox" is the same outcome recorded before the rename.
                    matches!(r.outcome.as_deref(), Some("gate:meta_hark" | "gate:meta_vox"))
                })
                .count() as u64;
            let agg = |key: &str| kinds.iter().find(|a| a.key == key);
            let avg = |c: f64, t: u64| if t > 0 { c / t as f64 } else { 0.0 };
            let (total_cost, total_in) = kinds.iter().fold((0.0, 0u64), |(c, t), a| {
                (c + a.cost_usd, t + a.usage.input + a.usage.cache_read + a.usage.cache_created)
            });
            let report = hark_core::domain::savings::compute(&hark_core::domain::savings::SavingsInputs {
                gate_blocked,
                gate_cost_usd: agg("gate").map(|a| a.cost_usd).unwrap_or(0.0),
                avg_worker_turn_usd: agg("worker").map(|a| avg(a.cost_usd, a.turns)).unwrap_or(0.0),
                local_answers: models.iter().find(|a| a.key == "local").map(|a| a.turns).unwrap_or(0),
                avg_ask_usd: agg("ask").map(|a| avg(a.cost_usd, a.turns)).unwrap_or(0.0),
                cache_read_tokens: kinds.iter().map(|a| a.usage.cache_read).sum(),
                usd_per_input_token: if total_in > 0 { total_cost / total_in as f64 } else { 0.0 },
            });
            println!("== evitado (janela {}) ==", if has("--day") { "24h" } else if has("--week") { "7d" } else { "toda" });
            println!("gate antes do worker      ${:.4}", report.avoided_gate_usd);
            println!("respostas locais          ${:.4}", report.avoided_local_usd);
            println!("cache lido (estimado)     ${:.4}", report.avoided_cache_usd);
            println!("TOTAL                     ${:.4}", report.total_usd);
            println!("\nmetodologia:");
            for m in &report.methodology {
                println!("- {m}");
            }
            return Ok(0);
        }
        let group = if has("--project") {
            SpendGroup::Workspace
        } else {
            SpendGroup::Kind
        };

        println!("== gasto medido (USD, turnos do Hark) ==");
        let live = store.spend_summary(&SpendQuery {
            since: since.clone(),
            group,
            source: SpendSource::Live,
            workspace: None,
        })?;
        if live.is_empty() {
            println!("(nenhum turno registrado ainda)");
        }
        for agg in &live {
            println!(
                "{:<40} ${:<9.4} {:>4} turnos {:>2} erros  in {} out {} cache {}",
                agg.key.chars().take(40).collect::<String>(),
                agg.cost_usd,
                agg.turns,
                agg.errors,
                agg.usage.input,
                agg.usage.output,
                agg.usage.cache_read,
            );
        }
        let total: f64 = live.iter().map(|a| a.cost_usd).sum::<f64>().max(0.0);
        println!("{:<40} ${total:.4}", "TOTAL");

        println!("\n== tokens do histórico (jsonl, máquina inteira) ==");
        for agg in store.spend_summary(&SpendQuery {
            since,
            group: SpendGroup::Model,
            source: SpendSource::Jsonl,
            workspace: None,
        })? {
            println!(
                "{:<40} in {:>10} out {:>10} cache_read {:>12} cache_new {:>10}",
                agg.key.chars().take(40).collect::<String>(),
                agg.usage.input,
                agg.usage.output,
                agg.usage.cache_read,
                agg.usage.cache_created,
            );
        }
        Ok(0)
    })();
    result.unwrap_or_else(|err| {
        eprintln!("hark: {err:#}");
        1
    })
}

fn cmd_use(name: &str) -> i32 {
    let config = Config::load();
    let known = name == "all" || config.context(name).is_some();
    if !known {
        eprintln!(
            "hark: unknown context '{name}'. Declared: all, {}",
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
            eprintln!("hark: {err:#}");
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

/// The cheap lane's runner, resolved as the app resolves it: `[agent] ask`
/// when set and usable, else the selected default, else claude when it is
/// here, else the first usable backend. Claude answers under a real
/// schema; an ACP agent is asked for JSON in the prompt. The tier the
/// caller asks for is said in the agent's own vocabulary.
fn ask_runner(config: &Config) -> Box<dyn hark_core::ports::AgentRunner + Send + Sync> {
    use hark_core::domain::agents;
    use hark_core::ports::AgentBackend as _;
    let entries = agents::merge(&config.agents);
    let detected = hark_core::adapters::agent_detect::detected(config, &entries);
    let preference = if config.agent.ask.is_empty() { &config.agent.plugin } else { &config.agent.ask };
    let id = agents::default_agent(&entries, &detected, preference).unwrap_or_else(|| "claude".into());
    let entry = agents::resolve(&entries, &id)
        .or_else(|| agents::resolve(&entries, "claude"))
        .cloned()
        .expect("claude is a builtin");
    let inner: Box<dyn hark_core::ports::AgentRunner + Send + Sync> = if entry.plugin == "claude" {
        let bin = if entry.cmd == "claude" { config.claude_bin_resolved() } else { entry.cmd.clone() };
        Box::new(ClaudeCli { claude_bin: bin, work_dir: config.data_dir(), envs: entry.env_pairs() })
    } else {
        hark_plugin_acp::AcpBackend::new(
            entry.id.clone(),
            entry.cmd.clone(),
            entry.args.clone(),
            entry.env_pairs(),
            entry.memory_file.clone(),
            entry.login_hint.clone(),
            config.data_dir(),
        )
        .runner()
    };
    Box::new(hark_core::app::runner::TieredRunner { inner, entry, tiers: config.models() })
}

/// Claude's files AND Hark's records of the agents that leave none.
struct BothHistories;

impl hark_core::ports::HistoryIndexer for BothHistories {
    fn refresh(
        &self,
        projects_dir: &std::path::Path,
        store: &mut dyn hark_core::ports::SessionStore,
    ) -> anyhow::Result<usize> {
        let files = refresh_index(projects_dir, store)?;
        let records = hark_core::adapters::recorder::refresh(&Config::load().data_dir(), store)?;
        Ok(files + records)
    }
}

fn cmd_ask(question: &str) -> i32 {
    let config = Config::load();
    let result = (|| -> anyhow::Result<_> {
        let mut store = open_store(&config)?;
        let state = state_file::load(&config.data_dir());
        let runner = ask_runner(&config);
        let mut deps = AskDeps {
            active_context: state.active_context,
            projects: state.projects,
            active_project: None,
            workers: state.workers,
            journal: &hark_core::adapters::memory_files::HarkDir,
            store: &mut store,
            live: &ClaudeAgentsCli {
                claude_bin: config.claude_bin_resolved(),
            },
            repos: &GitCli,
            runner: &*runner,
            config: &config,
            indexer: &BothHistories,
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
            match hark_core::domain::reply::VoiceReply::from_turn(&turn) {
                Some(reply) => {
                    println!("🔊 {}\n", reply.fala);
                    println!("{}", reply.detalhes);
                    for item in &reply.itens {
                        println!("  • {item}");
                    }
                }
                None => println!("{}", turn.raw),
            }
            eprintln!(
                "\n[{} · ${:.4}]",
                turn.model.as_deref().unwrap_or("?"),
                turn.cost_usd.unwrap_or(0.0)
            );
            0
        }
        Ok(turn) => {
            eprintln!("claude error: {}", turn.raw);
            1
        }
        Err(err) => {
            eprintln!("hark: {err:#}");
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
            projects: state.projects,
            active_project: None,
            workers: state.workers,
            journal: &hark_core::adapters::memory_files::HarkDir,
            store: &mut store,
            live: &ClaudeAgentsCli {
                claude_bin: config.claude_bin_resolved(),
            },
            repos: &GitCli,
            runner: &NoopRunner,
            config: &config,
            indexer: &BothHistories,
        };
        let (snapshot, topical) = hark_core::app::ask::snapshot_for_question(&mut deps, question)?;
        let budget = hark_core::domain::prompt::PromptBudget {
            max_chars: config.prompt_budget_chars,
        };
        Ok(hark_core::domain::prompt::build_budgeted(
            question, &snapshot, &topical, &budget,
        ))
    })();
    match result {
        Ok(prompt) => {
            println!("{prompt}");
            0
        }
        Err(err) => {
            eprintln!("hark: {err:#}");
            1
        }
    }
}

fn cmd_index(full: bool) -> i32 {
    let config = Config::load();
    if full {
        // Schema/behavior changes need a from-scratch pass.
        let _ = std::fs::remove_file(config.data_dir().join("index.db"));
    }
    match open_store(&config)
        .and_then(|mut store| {
            let files = refresh_index(&config.projects_dir, &mut store)?;
            let records = hark_core::adapters::recorder::refresh(&config.data_dir(), &mut store)?;
            Ok(files + records)
        })
    {
        Ok(n) => {
            println!("indexed {n} changed file(s)");
            0
        }
        Err(err) => {
            eprintln!("hark: {err:#}");
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
            projects: state.projects,
            active_project: None,
            workers: state.workers,
            journal: &hark_core::adapters::memory_files::HarkDir,
            store: &mut store,
            live: &ClaudeAgentsCli {
                claude_bin: config.claude_bin_resolved(),
            },
            repos: &GitCli,
            runner: &NoopRunner,
            config: &config,
            indexer: &BothHistories,
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
            eprintln!("hark: {err:#}");
            1
        }
    }
}

fn cmd_dispatch(instruction: &str, session_override: Option<&str>) -> i32 {
    use hark_core::adapters::memory_files;
    use hark_plugin_claude::worker;
    use hark_core::app::dispatch::{plan, Plan, Planned};
    use hark_plugin_claude::stream::PermissionDecision;
    use hark_core::domain::memory::{WorkerRecord, WorkerStatus};

    let config = Config::load();
    let planned = (|| -> anyhow::Result<Plan> {
        let mut store = open_store(&config)?;
        let state = state_file::load(&config.data_dir());
        let mut deps = AskDeps {
            active_context: state.active_context,
            projects: state.projects,
            active_project: None,
            workers: state.workers,
            journal: &hark_core::adapters::memory_files::HarkDir,
            store: &mut store,
            live: &ClaudeAgentsCli {
                claude_bin: config.claude_bin_resolved(),
            },
            repos: &GitCli,
            runner: &NoopRunner,
            config: &config,
            indexer: &BothHistories,
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
            eprintln!("Nenhuma sessão bate com essa instrução. Use --session <id> (veja hark sessions).");
            return 5;
        }
        Err(err) => {
            eprintln!("hark: {err:#}");
            return 1;
        }
    };

    // Feed the invisible kanban: this instruction is now a Doing task
    // linked to the target session.
    let board_note = |status: hark_core::domain::board::TaskStatus, nota: String| {
        if let Ok(mut store) = open_store(&config) {
            use hark_core::ports::SessionStore;
            let updates = [hark_core::domain::board::BoardUpdate {
                titulo: instruction.chars().take(60).collect(),
                status,
                sessao: None,
                nota: Some(nota),
            }];
            if let Ok(current) = store.board() {
                let merged = hark_core::domain::board::apply_updates(
                    current,
                    &updates,
                    &now_iso(),
                    Some(&planned.workspace_root.display().to_string()),
                    Some(&planned.session.session_id),
                );
                let _ = store.save_board(&merged);
            }
        }
    };
    board_note(
        hark_core::domain::board::TaskStatus::Doing,
        "despachado pelo hark".into(),
    );

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
        "# Hark dispatch {task_id}\n\n- session: {}\n- workspace: {}\n\n## Instruction\n\n{instruction}\n",
        planned.session.session_id,
        planned.workspace_root.display()
    );
    let _ = memory_files::write_brief(&planned.workspace_root, &task_id, &brief);

    // Register as running (machine-wide view).
    let mut state = state_file::load(&config.data_dir());
    state.workers.push(WorkerRecord {
        // hark-cli is pinned to the native plugin (F9.0).
        agent: "claude".into(),
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
        limits: config.spawn_limits_for(&planned.workspace_root),
        envs: {
            let settings = std::fs::read_to_string(hark_core::config::expand_home("~/.claude/settings.json")).unwrap_or_default();
            let status = hark_plugin_claude::eco::detect(&settings);
            hark_plugin_claude::eco::eco_envs(status, config.assist.ponytail.as_deref(), config.assist.caveman.as_deref(), config.assist.tokensave.as_deref())
        },
        directives: hark_core::domain::directives::parse(instruction),
        claude_bin: config.claude_bin_resolved(),
        cwd: planned.workspace_root.clone(),
        session_id: planned.session.session_id.clone(),
        instruction: instruction.to_string(),
        fork: false,
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

    // Ledger before anything else: even failed turns burned tokens.
    if let Ok(turn) = &result {
        use hark_core::ports::SpendLedger;
        if let Ok(mut store) = open_store(&config) {
            let workspace_str = planned.workspace_root.display().to_string();
            let rows = hark_core::domain::spend::rows_from_turn(
                &now_iso(),
                hark_core::domain::spend::SpendKind::Dispatch,
                &hark_core::domain::spend::SpendMeta {
                    task_id: Some(&task_id),
                    session_id: Some(&planned.session.session_id),
                    workspace: Some(&workspace_str),
                    ..Default::default()
                },
                turn,
            );
            let _ = store.record_spend(&rows);
        }
    }

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
    board_note(
        hark_core::domain::board::TaskStatus::Waiting,
        summary.chars().take(120).collect(),
    );
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
            eprintln!("hark: {err:#}");
            code
        }
    }
}

fn cmd_ps(clear: bool) -> i32 {
    use hark_core::domain::memory::WorkerStatus;
    let config = Config::load();
    let mut state = state_file::load(&config.data_dir());
    if clear {
        // Keep only running workers; finished history lives in .hark/state.md.
        let before = state.workers.len();
        state.workers.retain(|w| w.status == WorkerStatus::Running);
        let _ = state_file::save(&config.data_dir(), &state);
        println!("removed {} finished worker(s)", before - state.workers.len());
    }
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

fn cmd_board() -> i32 {
    use hark_core::ports::SessionStore;
    let config = Config::load();
    match open_store(&config).and_then(|store| store.board()) {
        Ok(tasks) if tasks.is_empty() => {
            println!("quadro vazio");
            0
        }
        Ok(tasks) => {
            println!("{}", hark_core::domain::board::render(&tasks));
            0
        }
        Err(err) => {
            eprintln!("hark: {err:#}");
            1
        }
    }
}

/// Download the default whisper model into the data dir.
/// Update both halves in one step: the CLI and the app bundle. The app
/// updates ITSELF from 0.2.5 on (it offers a restart when a build is
/// ready), so this exists for two cases — the hop from a build that
/// predates the updater, and anyone who turned auto_update off.
fn cmd_update() -> i32 {
    // The same public installer the docs point at, over TLS from our own
    // domain: one place decides how Hark is laid out on disk.
    const INSTALLER: &str = "curl -fsSL https://harkharness.web.app/install.sh | bash";
    println!("rodando o instalador publico: {INSTALLER}");
    match std::process::Command::new("/bin/bash").arg("-c").arg(INSTALLER).status() {
        Ok(status) if status.success() => 0,
        Ok(status) => {
            eprintln!("hark: instalador saiu com {status}");
            1
        }
        Err(err) => {
            eprintln!("hark: nao consegui rodar o instalador: {err}");
            1
        }
    }
}

fn cmd_setup(rest: &[String]) -> i32 {
    use hark_core::adapters::model_fetch;
    let config = Config::load();
    // `--model <key>` downloads a specific model and points the config at
    // it. Without it, setup only ensures the configured one exists.
    let wanted = rest
        .iter()
        .position(|a| a == "--model")
        .and_then(|i| rest.get(i + 1))
        .map(String::as_str);
    let path = config.whisper_model_path();
    if wanted.is_none() && path.exists() {
        println!("whisper model ok: {}", path.display());
        return 0;
    }
    let key = wanted.unwrap_or("small");
    let Some(model) = model_fetch::whisper_model(key) else {
        eprintln!("hark: modelo desconhecido: {key}");
        return 2;
    };
    let dir = config
        .data_dir()
        .join("models");
    let dir = &dir;
    println!("downloading {} ({})…", model.url, model.size_label);
    let mut last = 255u8;
    match model_fetch::download_model(model, dir, |pct| {
        if pct != last {
            last = pct;
            print!("\r{pct:>3}%");
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
    }) {
        Ok(path) => {
            println!("\ndone (sha256 verified): {}", path.display());
            // Point the config at what was actually downloaded, or asking
            // for large-v3-turbo would leave the mic on the old model.
            let cfg_path = hark_core::config::config_path();
            let text = std::fs::read_to_string(&cfg_path).unwrap_or_default();
            let patch = serde_json::json!({ "whisper_model": path.display().to_string() });
            match hark_core::config::patch_toml(&text, &patch) {
                Ok(out) => {
                    if let Some(parent) = cfg_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Err(err) = std::fs::write(&cfg_path, out) {
                        eprintln!("hark: config nao atualizado: {err}");
                    } else {
                        println!("config aponta para {key}");
                    }
                    0
                }
                Err(err) => {
                    eprintln!("hark: config nao atualizado: {err}");
                    1
                }
            }
        }
        Err(err) => {
            eprintln!("\nhark: {err}");
            1
        }
    }
}

fn load_voice(config: &Config) -> anyhow::Result<(hark_core::adapters::whisper_stt::WhisperStt, hark_core::adapters::say_tts::SayTts, hark_core::adapters::cpal_audio::CpalMic)> {
    use hark_core::adapters::{cpal_audio::CpalMic, say_tts::SayTts, whisper_stt::WhisperStt};
    eprint!("loading whisper… ");
    let stt = WhisperStt::load(&config.whisper_model_path(), &config.stt_language(), &config.vocab)?;
    stt.warmup();
    eprintln!("ready");
    Ok((stt, SayTts { voice: config.voice.clone() }, CpalMic::default()))
}

/// Mic test: record one utterance, print the transcript. No Claude, no cost.
fn cmd_hear() -> i32 {
    use hark_core::ports::{AudioIn, Cue, Stt, Tts};
    let config = Config::load();
    let (stt, tts, mic) = match load_voice(&config) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("hark: {err:#}");
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
            eprintln!("hark: {err:#}");
            1
        }
    }
}

/// The voice loop: hear -> route (ask|dispatch) -> speak.
fn cmd_listen() -> i32 {
    use hark_core::domain::intent::{route, Route};
    use hark_core::ports::{AudioIn, Cue, Stt, Tts};

    let config = Config::load();
    let (stt, tts, mic) = match load_voice(&config) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("hark: {err:#}");
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

    eprintln!("hark ouvindo. Diga \"sair\" para encerrar. Ctrl+C também funciona.");
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
                    // The strict verdict grammar: "assim que der" is not a yes.
                    Some(answer)
                        if hark_core::domain::verdict::interpret_permission(&answer)
                            .is_some_and(|(allow, _)| allow) =>
                    {
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
fn cmd_ask_spoken(question: &str, tts: &impl hark_core::ports::Tts) -> i32 {
    let config = Config::load();
    let result = (|| -> anyhow::Result<_> {
        let mut store = open_store(&config)?;
        let state = state_file::load(&config.data_dir());
        let runner = ask_runner(&config);
        let mut deps = AskDeps {
            active_context: state.active_context,
            projects: state.projects,
            active_project: None,
            workers: state.workers,
            journal: &hark_core::adapters::memory_files::HarkDir,
            store: &mut store,
            live: &ClaudeAgentsCli {
                claude_bin: config.claude_bin_resolved(),
            },
            repos: &GitCli,
            runner: &*runner,
            config: &config,
            indexer: &BothHistories,
        };
        ask(question, &mut deps, &mut |_| {})
    })();

    match result {
        Ok(turn) if !turn.is_error => {
            if let Some(reply) = hark_core::domain::reply::VoiceReply::from_turn(&turn) {
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
            eprintln!("hark: {err:#}");
            1
        }
    }
}

fn now_iso() -> String {
    use hark_core::chrono::{SecondsFormat, Utc};
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn chrono_compact() -> String {
    use hark_core::chrono::Utc;
    Utc::now().format("%m%d%H%M%S").to_string()
}

/// `sessions` never talks to Claude.
struct NoopRunner;
impl hark_core::ports::AgentRunner for NoopRunner {
    fn ask(
        &self,
        _request: &hark_core::ports::TurnRequest,
        _on_event: &mut dyn FnMut(&ClaudeEvent),
    ) -> anyhow::Result<hark_plugin_claude::stream::TurnResult> {
        anyhow::bail!("not used")
    }
}

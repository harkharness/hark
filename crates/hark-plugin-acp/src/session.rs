//! One live ACP session: the handshake, the prompt turns, the agent's
//! questions back to us, and the event stream the driver consumes.
//!
//! Plain threads and std pipes, like the claude plugin — no async
//! runtime. A reader thread turns every line from the agent into either
//! an answer to something we asked or an `AgentEvent`; everything Hark
//! sends is one line written under a mutex.
//!
//! Two ACP facts shape the design and are pinned by the tests below:
//!
//! - `agent_message_chunk` is a TOKEN-sized delta. Hark's transcript is
//!   message-sized (the claude plugin emits whole assistant messages), so
//!   chunks are coalesced into one `AssistantText` per segment — a segment
//!   ends when a tool call, a thought, a permission ask or the end of the
//!   turn arrives. `TurnResult.raw` is the LAST segment, which is what the
//!   window compares against to avoid a duplicate bubble.
//! - `usage_update` is per-session and CUMULATIVE for cost. `used` is the
//!   size of this turn's prompt (the same "prompt = context" reading
//!   `context_fill` makes of claude's input tokens), so it maps straight
//!   to `input` + `context_window`; `cost.amount` is a running total, so a
//!   turn's cost is the delta from the previous reading.

use crate::caps::{negotiate, Negotiated};
use crate::directives::{self as dirs, Offer};
use crate::rpc::{self, Incoming};
use crate::translate;
use hark_agent::{AgentEvent, AgentPhase, ModelUsage, PermissionDecision, TokenUsage, TurnResult};
use hark_core::domain::directives::Directives;
use hark_core::ports::{DirectivesApplied, EventRx, LiveDirectives};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// ACP v1 — the version gemini 0.46.0 answered (spikes/acp).
const PROTOCOL_VERSION: u64 = 1;
/// A handshake that takes longer than this is a hung agent, not a slow one.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(60);
/// Stderr lines kept for the exit report.
const STDERR_TAIL: usize = 20;

/// The two ends of the agent's stdio, whatever they are attached to: a
/// child process in the app, a pair of pipes in a test.
pub struct Wire {
    pub reader: Box<dyn std::io::Read + Send>,
    pub writer: Box<dyn std::io::Write + Send>,
}

/// What a new session needs to know before its first turn.
pub struct Opening<'a> {
    /// Registry id ("gemini"): labels the usage rows.
    pub agent: &'a str,
    pub cwd: &'a std::path::Path,
    /// Empty = a brand-new session; otherwise `session/load` this one —
    /// or `session/fork` it, when `fork` is set.
    pub session_id: &'a str,
    pub instruction: &'a str,
    /// Pasted screenshots riding on the opening prompt: (media type, base64).
    pub images: &'a [(String, String)],
    pub memory_file: Option<String>,
    /// Mode, model and effort the chat opens with — said to the agent in
    /// its own ids (`session/set_mode`, `session/set_config_option`)
    /// between session/new and the first word, for whatever it offers.
    pub directives: &'a Directives,
    /// The ceiling. ACP has no flag for it, so the session is the
    /// ceiling: past the budget or the round count hark cancels the turn
    /// and reports it as such. The Claude adapter also takes both as SDK
    /// options in `_meta`, so it enforces them itself as well.
    pub limits: &'a hark_agent::SpawnLimits,
    /// A one-shot structured question rather than a chat: the persona
    /// goes as the REAL system prompt where the agent takes one (the
    /// Claude adapter, via `_meta`), with settings and tools stripped —
    /// what makes the cheap lane cheap. Elsewhere the persona is prepended
    /// to the prompt, as before.
    pub lean: Option<LeanAsk<'a>>,
    /// Open a NEW session on this one's history ("open in parallel").
    /// `session/fork`, for an agent that announces
    /// `sessionCapabilities.fork`; refused by name on one that does not —
    /// loading the old session instead would not be a fork.
    pub fork: bool,
}

/// The cheap lane's opening: see `Opening::lean`.
#[derive(Debug, Clone, Copy)]
pub struct LeanAsk<'a> {
    pub system_prompt: &'a str,
}

/// A session that answered the handshake and took its opening prompt.
pub struct Connected {
    pub session: Arc<AcpSession>,
    pub events: EventRx,
    pub negotiated: Negotiated,
}

/// Handshake, create-or-load, opening prompt. Blocks until the agent has
/// answered `session/new` (or `session/load`); the opening prompt itself
/// streams through `events`.
///
/// Failures carry the health code the windows already know: an
/// unauthenticated agent is `agent_auth: <its own words>`.
pub fn connect(
    wire: Wire,
    child: Option<std::process::Child>,
    stderr: Option<std::process::ChildStderr>,
    opening: &Opening,
) -> anyhow::Result<Connected> {
    let (tx, rx) = mpsc::channel();
    let session = Arc::new(AcpSession {
        agent: opening.agent.to_string(),
        writer: Mutex::new(Some(wire.writer)),
        pid: child.as_ref().map(|c| c.id()),
        child: Mutex::new(child),
        session_id: Mutex::new(String::new()),
        next_id: AtomicU64::new(1),
        pending: Mutex::new(HashMap::new()),
        asks: Mutex::new(HashMap::new()),
        ask_seq: AtomicU64::new(1),
        stderr_tail: Arc::new(Mutex::new(VecDeque::new())),
        cost_before: Mutex::new(0.0),
        offer: Mutex::new(Offer::default()),
        limits: *opening.limits,
    });
    if let Some(stderr) = stderr {
        tail_stderr(stderr, session.stderr_tail.clone());
    }
    // `SessionStarted` goes out from here, before the reader owns the only
    // long-lived sender; this clone dies with the function.
    let started = tx.clone();
    spawn_reader(wire.reader, session.clone(), tx);

    match handshake(&session, opening, &started) {
        Ok(negotiated) => Ok(Connected { session, events: rx, negotiated }),
        Err(err) => {
            // A refused handshake must not leave an agent running.
            session.shutdown();
            Err(err)
        }
    }
}

/// initialize → session/new | session/load → SessionStarted → the opening
/// prompt. Separate so a failure anywhere can tear the process down.
fn handshake(
    session: &Arc<AcpSession>,
    opening: &Opening,
    started: &Sender<AgentEvent>,
) -> anyhow::Result<Negotiated> {
    let init = session
        .call(
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                // Declined on purpose: the agent uses its OWN tools to read
                // and write files and run commands. Hark serves neither
                // fs/* nor terminal/*, and saying so here is what keeps the
                // agent from asking (spikes/acp/FINDINGS.md).
                "clientCapabilities": {
                    "fs": { "readTextFile": false, "writeTextFile": false },
                    "terminal": false
                },
                "clientInfo": { "name": "hark", "version": env!("CARGO_PKG_VERSION") }
            }),
        )
        .map_err(|e| failure("initialize", e, session))?;
    let mut negotiated = negotiate(&init, opening.memory_file.clone());

    let resuming = !opening.session_id.is_empty();
    let forking = resuming && opening.fork;
    if forking && !negotiated.caps.fork {
        anyhow::bail!(
            "agent_failed: {} não anuncia session/fork — abrir em paralelo não existe nele",
            opening.agent
        );
    }
    let cwd = opening.cwd.display().to_string();
    let (method, params) = if forking {
        // A new session grown from the old one's history (the spec's
        // ForkSessionRequest): the answer is a session/new answer, new id
        // and offer included, and the Claude adapter takes the same _meta.
        let mut params = json!({ "sessionId": opening.session_id, "cwd": cwd, "mcpServers": [] });
        if let Some(meta) = claude_meta(opening, negotiated.claude_code) {
            params["_meta"] = meta;
        }
        ("session/fork", params)
    } else if resuming {
        ("session/load", json!({ "sessionId": opening.session_id, "cwd": cwd, "mcpServers": [] }))
    } else {
        let mut params = json!({ "cwd": cwd, "mcpServers": [] });
        if let Some(meta) = claude_meta(opening, negotiated.claude_code) {
            params["_meta"] = meta;
        }
        ("session/new", params)
    };
    let answer = session.call(method, params).map_err(|e| failure(method, e, session))?;
    let session_id = if resuming && !forking {
        opening.session_id.to_string()
    } else {
        answer
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("agent_failed: {method} answered without a sessionId"))?
            .to_string()
    };
    *session.session_id.lock().unwrap_or_else(|e| e.into_inner()) = session_id.clone();
    let _ = started.send(AgentEvent::SessionStarted { session_id, slash_commands: Vec::new() });

    // What the agent can be told: its modes and config options. The sheet
    // learns it here — nothing before session/new could know — and the
    // opening directives go on the wire before the first word, so a chat
    // opened in plan mode STARTS in plan mode.
    let offer = Offer::from_answer(&answer);
    let (mode, model, effort) = offer.directive_caps();
    negotiated.caps.directive_mode = mode;
    negotiated.caps.directive_model = model;
    negotiated.caps.directive_effort = effort;
    *session.offer.lock().unwrap_or_else(|e| e.into_inner()) = offer;
    if let Err(err) = session.apply_directives(opening.directives) {
        // A knob the agent refused is not a reason to lose the chat: the
        // pill shows the truth on the next reading, the turn goes on.
        if std::env::var_os("HARK_DEBUG").is_some() {
            eprintln!("[hark acp {}] opening directives: {err}", session.agent);
        }
    }

    // The persona: a real system prompt where the agent took one in
    // `_meta`, else the first thing the model reads.
    let first = match (&opening.lean, negotiated.claude_code) {
        (Some(lean), false) => format!("{}\n\n{}", lean.system_prompt.trim(), opening.instruction),
        _ => opening.instruction.to_string(),
    };
    session.prompt(&first, opening.images)?;
    Ok(negotiated)
}

/// What only the Claude adapter understands on `session/new`, and only
/// when its initialize said it is that adapter: a replacement system
/// prompt and Claude Agent SDK options. For a lean ask: our persona as
/// the system prompt, no settings, no tools, one turn — recorded: the
/// adapter's own prompt was 28.5k tokens of cache write for one word.
/// For any opening: the ceiling, so the SDK enforces it as the CLI's
/// `--max-budget-usd` / `--max-turns` would. None when nothing applies.
fn claude_meta(opening: &Opening, claude_code: bool) -> Option<Value> {
    if !claude_code {
        return None;
    }
    let mut meta = serde_json::Map::new();
    let mut options = serde_json::Map::new();
    if let Some(lean) = &opening.lean {
        meta.insert("systemPrompt".into(), json!(lean.system_prompt));
        meta.insert("disableBuiltInTools".into(), json!(true));
        options.insert("settingSources".into(), json!([]));
        options.insert("tools".into(), json!([]));
        options.insert("maxTurns".into(), json!(1));
    }
    if let Some(budget) = opening.limits.max_budget_usd.filter(|b| *b > 0.0) {
        options.insert("maxBudgetUsd".into(), json!(budget));
    }
    if let Some(turns) = opening.limits.max_turns.filter(|t| *t > 0) {
        options.entry("maxTurns".to_string()).or_insert(json!(turns));
    }
    if !options.is_empty() {
        meta.insert("claudeCode".into(), json!({ "options": options }));
    }
    (!meta.is_empty()).then_some(Value::Object(meta))
}

/// Why a handshake call did not get its answer.
enum CallError {
    /// The agent answered with a JSON-RPC error object.
    Rpc(Value),
    /// The stream closed (or was never writable) before an answer.
    Closed,
    Timeout,
}

/// One of our health codes, with the agent's own words attached. The
/// windows render `agent_auth:` as a login card and the rest as prose.
fn failure(method: &str, err: CallError, session: &AcpSession) -> anyhow::Error {
    match err {
        CallError::Rpc(err) => match translate::auth_error(&err) {
            Some(msg) => anyhow::anyhow!("{}{msg}", translate::AUTH_PREFIX),
            None => {
                let msg = err.get("message").and_then(Value::as_str).unwrap_or("erro sem mensagem");
                anyhow::anyhow!("agent_failed: {method}: {msg}")
            }
        },
        CallError::Closed => {
            let tail = session.stderr_tail.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect::<Vec<_>>().join(" ");
            anyhow::anyhow!("agent_failed: {method}: o agente fechou a conexão ({})", tail.trim())
        }
        CallError::Timeout => anyhow::anyhow!(
            "agent_failed: {method}: sem resposta em {}s",
            HANDSHAKE_TIMEOUT.as_secs()
        ),
    }
}

/// What we are waiting for under a request id.
enum Pending {
    /// A handshake call blocked on its answer.
    Reply(Sender<Result<Value, Value>>),
    /// The turn in flight: its answer is the turn's result.
    Prompt,
}

/// A `session/request_permission` the user has not answered yet.
struct OpenAsk {
    /// The agent's id for it, echoed back verbatim.
    rpc_id: Value,
    /// (optionId, kind) as offered.
    options: Vec<(String, String)>,
}

/// The last `usage_update` of the turn.
#[derive(Clone, Copy)]
struct UsageReading {
    used: u64,
    size: u64,
    /// The session's running total — the turn's share is a delta.
    total_cost: Option<f64>,
}

/// Reader-thread state for the turn in flight.
#[derive(Default)]
struct Turn {
    /// Chunks of the segment being written.
    segment: String,
    /// The last segment flushed — `TurnResult.raw`.
    last_segment: String,
    usage: Option<UsageReading>,
    /// Tool calls this turn asked for — the round count the ceiling
    /// measures, ACP having no turn counter of its own.
    tool_calls: u32,
    /// Why hark cancelled the turn, when it did: the result reports it
    /// as an error naming the numbers, never as a quiet stop.
    ceiling: Option<String>,
}

impl Turn {
    /// Close the segment: one message for however many chunks it took.
    fn flush(&mut self, tx: &Sender<AgentEvent>) {
        if self.segment.is_empty() {
            return;
        }
        self.last_segment = std::mem::take(&mut self.segment);
        let _ = tx.send(AgentEvent::AssistantText(self.last_segment.clone()));
    }
}

/// The live handle. Cheap to clone through the Arc the driver holds.
pub struct AcpSession {
    agent: String,
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    child: Mutex<Option<std::process::Child>>,
    pid: Option<u32>,
    session_id: Mutex<String>,
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, Pending>>,
    asks: Mutex<HashMap<String, OpenAsk>>,
    ask_seq: AtomicU64,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    cost_before: Mutex<f64>,
    /// Modes and config options the agent offered at session/new, with
    /// their current values as the agent last reported them.
    offer: Mutex<Offer>,
    /// The ceiling this session runs under (see `Opening::limits`).
    limits: hark_agent::SpawnLimits,
}

impl AcpSession {
    /// Say Hark's directives in the agent's ids, for whatever it offers:
    /// `session/set_mode` for the permission mode, `session/set_config_option`
    /// for model and effort. A knob the agent does not offer is skipped and
    /// reported as not applied; a knob it offers but whose requested VALUE
    /// is not on the list (a model name) is an error naming the list —
    /// the one case where silence would hide a real mismatch. Nothing is
    /// sent for a value already in force.
    pub fn apply_directives(&self, d: &Directives) -> anyhow::Result<LiveDirectives> {
        let offer = self.offer.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let session_id = self.session_id.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let mut applied = DirectivesApplied::default();

        if let (Some(mode), Some(modes)) = (d.mode, offer.modes.as_ref()) {
            if let Some(id) = dirs::mode_id(mode, modes) {
                if modes.current != id {
                    self.call("session/set_mode", json!({ "sessionId": session_id, "modeId": id }))
                        .map_err(|e| failure("session/set_mode", e, self))?;
                    let mut held = self.offer.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(m) = held.modes.as_mut() {
                        m.current = id;
                    }
                }
                applied.mode = true;
            }
        }

        let set_option = |option: &dirs::ConfigOption, value: String| -> anyhow::Result<()> {
            if option.current.as_deref() == Some(value.as_str()) {
                return Ok(());
            }
            let answer = self
                .call(
                    "session/set_config_option",
                    json!({ "sessionId": session_id, "configId": option.id, "value": value }),
                )
                .map_err(|e| failure("session/set_config_option", e, self))?;
            if let Some(list) = answer.get("configOptions") {
                self.offer.lock().unwrap_or_else(|e| e.into_inner()).refresh_options(list);
            }
            Ok(())
        };

        if let (Some(effort), Some(option)) = (d.effort, offer.effort_option()) {
            if let Some(value) = dirs::effort_value(effort, option) {
                set_option(option, value)?;
                applied.effort = true;
            }
        }

        if let (Some(model), Some(option)) = (d.model.as_deref(), offer.model_option()) {
            match dirs::model_value(model, option) {
                Some(value) => {
                    set_option(option, value)?;
                    applied.model = true;
                }
                None => {
                    let offered: Vec<&str> = option.values.iter().map(|v| v.id.as_str()).collect();
                    anyhow::bail!(
                        "o agente {} não oferece o modelo {model}; oferece: {}",
                        self.agent,
                        offered.join(", ")
                    );
                }
            }
        }

        Ok(LiveDirectives::Applied(applied))
    }

    /// The next user turn. One at a time: ACP allows a single prompt in
    /// flight per session, and Hark's outbox already queues the rest.
    pub fn send_text(&self, text: &str, images: &[(String, String)]) -> anyhow::Result<()> {
        self.prompt(text, images)
    }

    /// Answer `session/request_permission`. Allow picks `allow_once` —
    /// never `allow_always`: a standing rule is Hark's own to keep, per
    /// window, and must not be planted in the agent behind the user.
    pub fn respond_permission(&self, request_id: &str, decision: PermissionDecision) -> anyhow::Result<()> {
        let ask = self
            .asks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(request_id)
            .ok_or_else(|| anyhow::anyhow!("nenhuma permissão {request_id} em aberto"))?;
        let pick = |kinds: &[&str]| {
            kinds.iter().find_map(|wanted| {
                ask.options.iter().find(|(_, kind)| kind == wanted).map(|(id, _)| id.clone())
            })
        };
        let chosen = match decision {
            // allow_always only when the agent offers nothing narrower —
            // the user did say yes.
            PermissionDecision::Allow => pick(&["allow_once", "allow_always"]),
            PermissionDecision::Deny => pick(&["reject_once", "reject_always"]),
        };
        let outcome = match chosen {
            Some(option_id) => json!({ "outcome": { "outcome": "selected", "optionId": option_id } }),
            None => json!({ "outcome": { "outcome": "cancelled" } }),
        };
        self.write(&rpc::response(&ask.rpc_id, outcome))
    }

    /// `session/cancel`, plus the answer the spec REQUIRES for every
    /// permission still open: `cancelled`.
    pub fn interrupt(&self) -> anyhow::Result<()> {
        let open: Vec<OpenAsk> = self
            .asks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain()
            .map(|(_, ask)| ask)
            .collect();
        for ask in open {
            let _ = self.write(&rpc::response(&ask.rpc_id, json!({ "outcome": { "outcome": "cancelled" } })));
        }
        let session_id = self.session_id.lock().unwrap_or_else(|e| e.into_inner()).clone();
        self.write(&rpc::notification("session/cancel", json!({ "sessionId": session_id })))
    }

    /// Hang up: EOF on the agent's stdin, then the process goes.
    pub fn shutdown(&self) {
        self.writer.lock().unwrap_or_else(|e| e.into_inner()).take();
        let mut child = self.child.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(child) = child.as_mut() {
            // A moment to leave on its own, then the safety net.
            std::thread::sleep(Duration::from_millis(300));
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Exit code and the agent's last words on stderr, once the stream
    /// has closed. Pipes-only sessions (tests) have neither.
    pub fn exit_report(&self) -> (Option<i32>, String) {
        let code = self
            .child
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
            .and_then(|c| c.wait().ok())
            .and_then(|status| status.code());
        let tail = self
            .stderr_tail
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        (code, tail)
    }

    fn write(&self, line: &str) -> anyhow::Result<()> {
        trace(">>", line);
        let mut guard = self.writer.lock().unwrap_or_else(|e| e.into_inner());
        let writer = guard.as_mut().ok_or_else(|| anyhow::anyhow!("sessão já encerrada"))?;
        writeln!(writer, "{line}")?;
        writer.flush()?;
        Ok(())
    }

    /// A request that blocks on its answer (handshake only).
    fn call(&self, method: &str, params: Value) -> Result<Value, CallError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (reply_tx, reply_rx) = mpsc::channel();
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).insert(id, Pending::Reply(reply_tx));
        if self.write(&rpc::request(id, method, params)).is_err() {
            self.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
            return Err(CallError::Closed);
        }
        match reply_rx.recv_timeout(HANDSHAKE_TIMEOUT) {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(error)) => Err(CallError::Rpc(error)),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
                Err(CallError::Timeout)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(CallError::Closed),
        }
    }

    /// The turn: text first, then every pasted image as its own block.
    fn prompt(&self, text: &str, images: &[(String, String)]) -> anyhow::Result<()> {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        if pending.values().any(|p| matches!(p, Pending::Prompt)) {
            anyhow::bail!("a turn is already in flight on this session");
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        pending.insert(id, Pending::Prompt);
        drop(pending);
        let mut blocks = vec![json!({ "type": "text", "text": text })];
        for (mime, data) in images {
            blocks.push(json!({ "type": "image", "data": data, "mimeType": mime }));
        }
        let session_id = self.session_id.lock().unwrap_or_else(|e| e.into_inner()).clone();
        self.write(&rpc::request(id, "session/prompt", json!({ "sessionId": session_id, "prompt": blocks })))
    }

    /// The agent asked whether a tool may run: remember its options under
    /// an id of OURS, and ask the user.
    fn on_permission(&self, rpc_id: Value, params: &Value, tx: &Sender<AgentEvent>) {
        let tool = params.get("toolCall").cloned().unwrap_or(Value::Null);
        let tool_name = tool
            .get("title")
            .or_else(|| tool.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or("tool")
            .to_string();
        let input = tool.get("rawInput").map(|i| i.to_string()).unwrap_or_default();
        let options = params
            .get("options")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|o| {
                        Some((
                            o.get("optionId")?.as_str()?.to_string(),
                            o.get("kind").and_then(Value::as_str).unwrap_or("").to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let request_id = format!("acp-{}", self.ask_seq.fetch_add(1, Ordering::Relaxed));
        self.asks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(request_id.clone(), OpenAsk { rpc_id, options });
        let _ = tx.send(AgentEvent::PermissionRequest { request_id, tool_name, input });
    }

    /// `session/update`: prose chunks accumulate; anything else closes the
    /// segment first, so the transcript keeps the order things happened.
    fn on_update(&self, params: &Value, turn: &mut Turn, tx: &Sender<AgentEvent>) {
        match translate::update(params) {
            AgentEvent::AssistantText(text) => {
                if turn.segment.is_empty() {
                    let _ = tx.send(AgentEvent::Status(AgentPhase::Writing));
                }
                turn.segment.push_str(&text);
            }
            AgentEvent::Ignored => {
                let update = params.get("update").unwrap_or(params);
                match update.get("sessionUpdate").and_then(Value::as_str).unwrap_or("") {
                    // The agent's own slash commands: the same event the
                    // claude plugin sends from its init line.
                    "available_commands_update" => {
                        let names: Vec<String> = update
                            .get("availableCommands")
                            .and_then(Value::as_array)
                            .map(|cmds| {
                                cmds.iter()
                                    .filter_map(|c| c.get("name").and_then(Value::as_str).map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        let session_id = self.session_id.lock().unwrap_or_else(|e| e.into_inner()).clone();
                        let _ = tx.send(AgentEvent::SessionStarted { session_id, slash_commands: names });
                    }
                    "usage_update" => {
                        // Agents price the turn on ONE update (the SDK
                        // result) and keep reporting context on others,
                        // with no cost field. The latest reading wins for
                        // used/size; the cost is the last one stated.
                        let stated = update.get("cost").and_then(|c| c.get("amount")).and_then(Value::as_f64);
                        turn.usage = Some(UsageReading {
                            used: update.get("used").and_then(Value::as_u64).unwrap_or(0),
                            size: update.get("size").and_then(Value::as_u64).unwrap_or(0),
                            total_cost: stated.or(turn.usage.and_then(|u| u.total_cost)),
                        });
                        if let Some(total) = stated {
                            self.hold_the_budget(total, turn);
                        }
                    }
                    // The agent moved a knob itself (or confirmed ours):
                    // the offer keeps the values in force, so a later
                    // request for the same value costs no call.
                    "current_mode_update" => {
                        if let Some(id) = update.get("currentModeId").and_then(Value::as_str) {
                            let mut held = self.offer.lock().unwrap_or_else(|e| e.into_inner());
                            if let Some(m) = held.modes.as_mut() {
                                m.current = id.to_string();
                            }
                        }
                    }
                    "config_option_update" => {
                        if let Some(list) = update.get("configOptions") {
                            self.offer.lock().unwrap_or_else(|e| e.into_inner()).refresh_options(list);
                        }
                    }
                    _ => {}
                }
            }
            other => {
                if matches!(other, AgentEvent::ToolUse { .. }) {
                    turn.tool_calls += 1;
                    self.hold_the_rounds(turn);
                }
                turn.flush(tx);
                let _ = tx.send(other);
            }
        }
    }

    /// Past the budget: cancel the turn, and remember why for its result.
    /// The CLI's `--max-budget-usd` has no ACP equivalent, so the client
    /// is the ceiling; the number compared is the session's running total,
    /// which is what the flag caps too.
    fn hold_the_budget(&self, total: f64, turn: &mut Turn) {
        let Some(cap) = self.limits.max_budget_usd.filter(|b| *b > 0.0) else { return };
        if total > cap && turn.ceiling.is_none() {
            turn.ceiling = Some(format!(
                "teto de US$ {cap:.2} atingido — a sessão já gastou US$ {total:.2}. \
                 Reabra a thread com um teto maior, ou \"sem teto\" na config."
            ));
            let _ = self.interrupt();
        }
    }

    /// Past the round count: same cut. Tool calls stand in for the CLI's
    /// agentic turns — each is one round trip the model asked for.
    fn hold_the_rounds(&self, turn: &mut Turn) {
        let Some(cap) = self.limits.max_turns.filter(|t| *t > 0) else { return };
        if turn.tool_calls > cap && turn.ceiling.is_none() {
            turn.ceiling = Some(format!(
                "teto de {cap} rodadas de ferramenta atingido neste turno ({} chamadas). \
                 Reabra a thread com um teto maior, ou \"sem teto\" na config.",
                turn.tool_calls
            ));
            let _ = self.interrupt();
        }
    }

    /// The prompt's answer, as the turn's result.
    fn finish_turn(&self, turn: &mut Turn, result: Option<Value>, error: Option<Value>) -> TurnResult {
        let cut = turn.ceiling.take();
        turn.tool_calls = 0;
        let (is_error, raw) = match error {
            Some(err) => {
                let msg = err.get("message").and_then(Value::as_str).unwrap_or("erro sem mensagem").to_string();
                let raw = match translate::auth_error(&err) {
                    Some(_) => format!("{}{msg}", translate::AUTH_PREFIX),
                    None => msg,
                };
                (true, raw)
            }
            None => {
                let stop = result
                    .as_ref()
                    .and_then(|r| r.get("stopReason"))
                    .and_then(Value::as_str)
                    .unwrap_or("end_turn")
                    .to_string();
                let text = std::mem::take(&mut turn.last_segment);
                // A turn with no prose says why it stopped, unless it was
                // the ordinary end or the stop the user asked for.
                let raw = if text.is_empty() && stop != "end_turn" && stop != "cancelled" {
                    format!("({stop})")
                } else {
                    text
                };
                (false, raw)
            }
        };
        // A turn hark cut for its ceiling is an error that names the
        // numbers, whatever the agent said about why it stopped.
        let (is_error, raw) = match cut {
            Some(reason) => (true, reason),
            None => (is_error, raw),
        };
        // The turn's own tokens ride the prompt answer when the adapter
        // sends them (claude-agent-acp: inputTokens / outputTokens /
        // cachedReadTokens / cachedWriteTokens — recorded); without them the
        // context reading stands in as "input". The model that actually ran
        // is in the answer's _meta when the adapter says (quota.model_usage);
        // else the turn is signed by the agent id.
        let answer_usage = result.as_ref().and_then(|r| r.get("usage")).map(|u| {
            let n = |key: &str| u.get(key).and_then(Value::as_u64).unwrap_or(0);
            TokenUsage {
                input: n("inputTokens"),
                output: n("outputTokens"),
                cache_read: n("cachedReadTokens"),
                cache_created: n("cachedWriteTokens"),
            }
        });
        let model = result
            .as_ref()
            .and_then(|r| r.pointer("/_meta/quota/model_usage/0/model"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| self.agent.clone());
        let reading = turn.usage.take();
        let cost_usd = reading.and_then(|r| r.total_cost).map(|total| {
            let mut before = self.cost_before.lock().unwrap_or_else(|e| e.into_inner());
            let share = (total - *before).max(0.0);
            *before = total;
            share
        });
        let usage = match (reading, answer_usage) {
            (None, None) => Vec::new(),
            (reading, answer) => vec![ModelUsage {
                model: model.clone(),
                usage: answer.unwrap_or_else(|| TokenUsage {
                    input: reading.map(|r| r.used).unwrap_or(0),
                    ..Default::default()
                }),
                cost_usd,
                context_window: reading.map(|r| r.size),
            }],
        };
        turn.segment.clear();
        TurnResult {
            is_error,
            reply: None,
            raw,
            cost_usd,
            duration_ms: None,
            model: Some(model),
            usage,
        }
    }
}

/// Every line from the agent, sorted and acted on. Owns the only
/// long-lived event sender: when the agent's stdout closes, this thread
/// ends, the sender drops, and the driver's `rx.iter()` ends with it.
fn spawn_reader(reader: Box<dyn std::io::Read + Send>, session: Arc<AcpSession>, tx: Sender<AgentEvent>) {
    std::thread::spawn(move || {
        let debug = std::env::var_os("HARK_DEBUG").is_some();
        let mut turn = Turn::default();
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            trace("<<", &line);
            if debug {
                eprintln!("[hark acp {}] {line}", session.agent);
            }
            let Some(msg) = rpc::parse(&line) else { continue };
            match msg {
                Incoming::Response { id, result, error } => {
                    let pending = session.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
                    match pending {
                        Some(Pending::Reply(reply)) => {
                            let _ = reply.send(match (result, error) {
                                (_, Some(err)) => Err(err),
                                (Some(res), None) => Ok(res),
                                (None, None) => Ok(Value::Null),
                            });
                        }
                        Some(Pending::Prompt) => {
                            turn.flush(&tx);
                            let result = session.finish_turn(&mut turn, result, error);
                            let _ = tx.send(AgentEvent::Result(result));
                        }
                        // An answer to nothing we asked (a timed-out call).
                        None => {}
                    }
                }
                Incoming::Notification { method, params } if method == "session/update" => {
                    session.on_update(&params, &mut turn, &tx);
                }
                Incoming::Notification { .. } => {}
                Incoming::Request { id, method, params } => match method.as_str() {
                    "session/request_permission" => {
                        turn.flush(&tx);
                        session.on_permission(id, &params, &tx);
                    }
                    // fs/*, terminal/*, elicitation/*: declined at initialize,
                    // refused here if asked anyway. The turn goes on.
                    _ => {
                        let _ = session.write(&rpc::error_response(
                            &id,
                            -32601,
                            &format!("hark does not serve {method}"),
                        ));
                    }
                },
            }
        }
        // EOF: whoever is blocked on an answer learns the stream is gone.
        turn.flush(&tx);
        session.pending.lock().unwrap_or_else(|e| e.into_inner()).clear();
        session.asks.lock().unwrap_or_else(|e| e.into_inner()).clear();
    });
}

/// The agent's stderr, last lines only, for the exit report.
fn tail_stderr(stderr: std::process::ChildStderr, tail: Arc<Mutex<VecDeque<String>>>) {
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let mut tail = tail.lock().unwrap_or_else(|e| e.into_inner());
            if tail.len() >= STDERR_TAIL {
                tail.pop_front();
            }
            tail.push_back(line);
        }
    });
}

/// `HARK_ACP_TRACE=<file>` records the wire both ways — how fixtures are
/// born (spikes/acp/FINDINGS.md: traffic, not the spec, is the source).
fn trace(direction: &str, line: &str) {
    let Some(path) = std::env::var_os("HARK_ACP_TRACE") else { return };
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{direction} {line}");
    }
}

impl hark_core::ports::AgentSession for AcpSession {
    fn send_text(&self, text: &str, images: &[(String, String)]) -> anyhow::Result<()> {
        AcpSession::send_text(self, text, images)
    }
    fn set_directives(&self, directives: &Directives) -> anyhow::Result<LiveDirectives> {
        AcpSession::apply_directives(self, directives)
    }
    fn respond_permission(&self, request_id: &str, decision: PermissionDecision) -> anyhow::Result<()> {
        AcpSession::respond_permission(self, request_id, decision)
    }
    fn interrupt(&self) -> anyhow::Result<()> {
        AcpSession::interrupt(self)
    }
    fn shutdown(&self) {
        AcpSession::shutdown(self)
    }
    fn pid(&self) -> Option<u32> {
        AcpSession::pid(self)
    }
    fn exit_report(&self) -> (Option<i32>, String) {
        AcpSession::exit_report(self)
    }
}

/// A scripted agent on the far end of two pipes, for this crate's tests:
/// the session's own suite and the one-shot ask's.
#[cfg(test)]
pub(crate) mod fake {
    use super::Wire;
    use crate::rpc::{self, Incoming};
    use serde_json::{json, Value};
    use std::io::{BufRead, BufReader, Write};
    use std::sync::{Arc, Mutex};

    /// The real `initialize` answer from gemini-cli 0.46.0 on this machine.
    pub const GEMINI_INIT: &str = include_str!("../fixtures/initialize.gemini-0.46.0.json");

    /// Everything hark sent, in order — the assertions read it back.
    pub type Seen = Arc<Mutex<Vec<Incoming>>>;

    /// `script` sees every message hark sends and gets a `say` to write
    /// one line back. Runs until hark hangs up.
    pub fn fake_agent(
        script: impl FnMut(&Incoming, &mut dyn FnMut(String)) + Send + 'static,
    ) -> (Wire, Seen) {
        let (agent_reads, hark_writes) = std::io::pipe().expect("pipe");
        let (hark_reads, agent_writes) = std::io::pipe().expect("pipe");
        let seen: Seen = Arc::default();
        let seen2 = seen.clone();
        std::thread::spawn(move || {
            let mut out = agent_writes;
            let mut script = script;
            for line in BufReader::new(agent_reads).lines().map_while(Result::ok) {
                if let Some(msg) = rpc::parse(&line) {
                    seen2.lock().unwrap().push(msg.clone());
                    let mut say = |s: String| {
                        let _ = writeln!(out, "{s}");
                    };
                    script(&msg, &mut say);
                }
            }
        });
        (Wire { reader: Box::new(hark_reads), writer: Box::new(hark_writes) }, seen)
    }

    /// An agent that handshakes like gemini and hands every prompt to
    /// `on_prompt(prompt request id, params, say)`.
    pub fn gemini_like(
        mut on_prompt: impl FnMut(u64, &Value, &mut dyn FnMut(String)) + Send + 'static,
    ) -> impl FnMut(&Incoming, &mut dyn FnMut(String)) + Send + 'static {
        move |msg, say| {
            if let Incoming::Request { id, method, params } = msg {
                match method.as_str() {
                    "initialize" => say(rpc::response(id, serde_json::from_str(GEMINI_INIT).unwrap())),
                    "session/new" => say(rpc::response(id, json!({ "sessionId": "s-1" }))),
                    "session/load" => say(rpc::response(id, json!({}))),
                    "session/prompt" => on_prompt(id.as_u64().unwrap(), params, say),
                    _ => say(rpc::error_response(id, -32601, "not in this script")),
                }
            }
        }
    }

    /// The real answers of claude-agent-acp 0.76.0 (recorded 14/09/2026).
    pub const CLAUDE_INIT: &str = include_str!("../fixtures/initialize.claude-agent-acp-0.76.0.json");
    pub const CLAUDE_NEW: &str = include_str!("../fixtures/session-new.claude-agent-acp-0.76.0.json");

    /// An agent that handshakes exactly like claude-agent-acp — its
    /// initialize (with the `_meta.claudeCode` signature) and its
    /// session/new offer — takes set_mode / set_config_option, and hands
    /// prompts to `on_prompt`. Both `session/new` params and the prompt
    /// land in `Seen` for the tests to inspect.
    pub fn claude_like(
        on_prompt: impl FnMut(u64, &Value, &mut dyn FnMut(String)) + Send + 'static,
    ) -> impl FnMut(&Incoming, &mut dyn FnMut(String)) + Send + 'static {
        let mut inner = offering(serde_json::from_str(CLAUDE_NEW).unwrap(), on_prompt);
        move |msg, say| {
            if let Incoming::Request { id, method, .. } = msg {
                if method == "initialize" {
                    say(rpc::response(id, serde_json::from_str(CLAUDE_INIT).unwrap()));
                    return;
                }
            }
            inner(msg, say)
        }
    }

    /// An agent whose session/new answers with `offer` (modes, config
    /// options) and takes `session/set_mode` / `session/set_config_option`,
    /// answering the latter with the list it was given back.
    pub fn offering(
        offer: Value,
        mut on_prompt: impl FnMut(u64, &Value, &mut dyn FnMut(String)) + Send + 'static,
    ) -> impl FnMut(&Incoming, &mut dyn FnMut(String)) + Send + 'static {
        move |msg, say| {
            if let Incoming::Request { id, method, params } = msg {
                match method.as_str() {
                    "initialize" => say(rpc::response(id, serde_json::from_str(GEMINI_INIT).unwrap())),
                    "session/new" => say(rpc::response(id, offer.clone())),
                    "session/set_mode" => say(rpc::response(id, json!({}))),
                    "session/set_config_option" => say(rpc::response(
                        id,
                        json!({ "configOptions": offer.get("configOptions").cloned().unwrap_or(json!([])) }),
                    )),
                    "session/prompt" => on_prompt(id.as_u64().unwrap(), params, say),
                    _ => say(rpc::error_response(id, -32601, "not in this script")),
                }
            }
        }
    }

    pub fn update(update: Value) -> String {
        rpc::notification("session/update", json!({ "sessionId": "s-1", "update": update }))
    }
    pub fn chunk(text: &str) -> Value {
        json!({ "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": text } })
    }
    pub fn end_turn(id: u64) -> String {
        rpc::response(&json!(id), json!({ "stopReason": "end_turn" }))
    }
    /// The prompt answer as claude-agent-acp 0.76 sends it: the turn's own
    /// token breakdown rides the response (recorded 14/09/2026).
    pub const CLAUDE_PROMPT_RESPONSE: &str =
        include_str!("../fixtures/prompt-response.claude-agent-acp-0.76.0.json");
    pub fn end_turn_with_usage(id: u64) -> String {
        rpc::response(&json!(id), serde_json::from_str(CLAUDE_PROMPT_RESPONSE).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::fake::{claude_like, end_turn_with_usage, offering};
    use super::*;
    use crate::rpc::{self, Incoming};
    use hark_agent::{AgentPhase, ModelUsage, TokenUsage};
    use hark_core::domain::directives::{Effort, Mode};
    use serde_json::{json, Value};
    use std::io::{BufRead, BufReader, Write};
    use std::sync::Mutex;
    use std::time::Duration;

    /// The real `initialize` answer from gemini-cli 0.46.0 on this machine.
    const GEMINI_INIT: &str = include_str!("../fixtures/initialize.gemini-0.46.0.json");

    /// Everything hark sent, in order — the assertions read it back.
    type Seen = Arc<Mutex<Vec<Incoming>>>;

    /// A scripted agent on the far end of two pipes. `script` sees every
    /// message hark sends and gets a `say` to write one line back.
    fn fake_agent(
        script: impl FnMut(&Incoming, &mut dyn FnMut(String)) + Send + 'static,
    ) -> (Wire, Seen) {
        let (agent_reads, hark_writes) = std::io::pipe().expect("pipe");
        let (hark_reads, agent_writes) = std::io::pipe().expect("pipe");
        let seen: Seen = Arc::default();
        let seen2 = seen.clone();
        std::thread::spawn(move || {
            let mut out = agent_writes;
            let mut script = script;
            for line in BufReader::new(agent_reads).lines().map_while(Result::ok) {
                if let Some(msg) = rpc::parse(&line) {
                    seen2.lock().unwrap().push(msg.clone());
                    let mut say = |s: String| {
                        let _ = writeln!(out, "{s}");
                    };
                    script(&msg, &mut say);
                }
            }
            // hark hung up: dropping `out` closes hark's reader in turn.
        });
        (Wire { reader: Box::new(hark_reads), writer: Box::new(hark_writes) }, seen)
    }

    /// An agent that handshakes like gemini and hands every prompt to
    /// `on_prompt(prompt request id, params, say)`.
    fn gemini_like(
        mut on_prompt: impl FnMut(u64, &Value, &mut dyn FnMut(String)) + Send + 'static,
    ) -> impl FnMut(&Incoming, &mut dyn FnMut(String)) + Send + 'static {
        move |msg, say| {
            if let Incoming::Request { id, method, params } = msg {
                match method.as_str() {
                    "initialize" => say(rpc::response(id, serde_json::from_str(GEMINI_INIT).unwrap())),
                    "session/new" => say(rpc::response(id, json!({ "sessionId": "s-1" }))),
                    "session/load" => say(rpc::response(id, json!({}))),
                    "session/prompt" => on_prompt(id.as_u64().unwrap(), params, say),
                    _ => say(rpc::error_response(id, -32601, "not in this script")),
                }
            }
        }
    }

    fn update(update: Value) -> String {
        rpc::notification("session/update", json!({ "sessionId": "s-1", "update": update }))
    }
    fn chunk(text: &str) -> Value {
        json!({ "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": text } })
    }
    fn end_turn(id: u64) -> String {
        rpc::response(&json!(id), json!({ "stopReason": "end_turn" }))
    }
    static NO_DIRECTIVES: Directives = Directives { mode: None, effort: None, model: None };

    fn opening<'a>(session_id: &'a str, instruction: &'a str) -> Opening<'a> {
        opening_with(session_id, instruction, &NO_DIRECTIVES)
    }

    fn opening_with<'a>(session_id: &'a str, instruction: &'a str, directives: &'a Directives) -> Opening<'a> {
        Opening {
            agent: "gemini",
            cwd: std::path::Path::new("/tmp/proj"),
            session_id,
            instruction,
            images: &[],
            memory_file: Some("GEMINI.md".into()),
            directives,
            limits: &NO_LIMITS,
            lean: None,
            fork: false,
        }
    }

    /// What claude-agent-acp answers to session/new, per its source —
    /// synthetic (the adapter was not installed when this was written).
    fn claude_offer() -> Value {
        json!({
            "sessionId": "s-1",
            "modes": { "currentModeId": "default", "availableModes": [
                { "id": "default", "name": "Always Ask" }, { "id": "acceptEdits", "name": "Accept Edits" },
                { "id": "plan", "name": "Plan Mode" }, { "id": "bypassPermissions", "name": "Bypass" } ] },
            "configOptions": [
                { "id": "model", "name": "Model", "category": "model", "type": "select", "currentValue": "default",
                  "options": [ { "value": "default", "name": "Default" }, { "value": "claude-haiku-4-5", "name": "Haiku 4.5" },
                               { "value": "claude-opus-4-6", "name": "Opus 4.6" } ] },
                { "id": "effort", "name": "Effort", "category": "thought_level", "type": "select", "currentValue": "default",
                  "options": [ { "value": "default", "name": "Default" }, { "value": "low", "name": "low" },
                               { "value": "high", "name": "high" }, { "value": "max", "name": "max" } ] }
            ]
        })
    }

    /// Requests hark sent with this method, in order, with their params.
    fn requests(seen: &Seen, method: &str) -> Vec<Value> {
        seen.lock()
            .unwrap()
            .iter()
            .filter_map(|m| match m {
                Incoming::Request { method: got, params, .. } if got == method => Some(params.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn opening_directives_reach_the_agent_before_the_first_word() {
        // A chat opened in plan mode, low effort, on the light model must
        // START that way: the knobs go on the wire between session/new and
        // the opening prompt, each in the agent's own vocabulary.
        let (wire, seen) = fake_agent(offering(claude_offer(), |id, _p, say| say(end_turn(id))));
        let wanted = Directives { mode: Some(Mode::Plan), effort: Some(Effort::Low), model: Some("haiku".into()) };
        let c = connect(wire, None, None, &opening_with("", "oi", &wanted)).expect("connects");
        let _ = until_result(&c.events);

        let order = methods(&seen);
        let prompt_at = order.iter().position(|m| m == "session/prompt").expect("prompt sent");
        let mode_at = order.iter().position(|m| m == "session/set_mode").expect("set_mode sent");
        let cfg_at = order.iter().position(|m| m == "session/set_config_option").expect("config sent");
        assert!(mode_at < prompt_at && cfg_at < prompt_at, "{order:?}");

        assert_eq!(requests(&seen, "session/set_mode")[0]["modeId"], "plan");
        let cfgs = requests(&seen, "session/set_config_option");
        let by_id: std::collections::BTreeMap<String, String> = cfgs
            .iter()
            .map(|p| (p["configId"].as_str().unwrap().to_string(), p["value"].as_str().unwrap().to_string()))
            .collect();
        assert_eq!(by_id.get("effort").map(String::as_str), Some("low"));
        assert_eq!(by_id.get("model").map(String::as_str), Some("claude-haiku-4-5"));
        for p in &cfgs {
            assert_eq!(p["sessionId"], "s-1");
        }
    }

    #[test]
    fn the_offer_fills_the_directive_sheet() {
        let (wire, _seen) = fake_agent(offering(claude_offer(), |id, _p, say| say(end_turn(id))));
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        let caps = &c.negotiated.caps;
        assert!(caps.directive_mode && caps.directive_model && caps.directive_effort);

        let (wire, _seen) = fake_agent(gemini_like(|id, _p, say| say(end_turn(id))));
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        let caps = &c.negotiated.caps;
        assert!(!caps.directive_mode && !caps.directive_model && !caps.directive_effort);
    }

    #[test]
    fn a_live_mode_change_is_one_set_mode_call_not_a_reopen() {
        let (wire, seen) = fake_agent(offering(claude_offer(), |id, _p, say| say(end_turn(id))));
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        let _ = until_result(&c.events);
        assert!(requests(&seen, "session/set_mode").is_empty(), "no directive, no call");

        let applied = c
            .session
            .apply_directives(&Directives { mode: Some(Mode::AcceptEdits), effort: None, model: None })
            .expect("applies");
        assert_eq!(applied, LiveDirectives::Applied(DirectivesApplied { mode: true, model: false, effort: false }));
        wait_for(&seen, |m| {
            matches!(m, Incoming::Request { method, params, .. }
                if method == "session/set_mode" && params["modeId"] == "acceptEdits")
        });
        assert_eq!(methods(&seen).iter().filter(|m| *m == "session/new").count(), 1, "the session was not reopened");

        // Asking for the mode already in force is not another call.
        let again = c
            .session
            .apply_directives(&Directives { mode: Some(Mode::AcceptEdits), effort: None, model: None })
            .expect("applies");
        assert_eq!(again, LiveDirectives::Applied(DirectivesApplied { mode: true, model: false, effort: false }));
        assert_eq!(requests(&seen, "session/set_mode").len(), 1);
    }

    #[test]
    fn an_agent_offering_no_knobs_is_asked_for_none() {
        // gemini_like answers session/new with a bare sessionId: no modes,
        // no config options. Nothing is sent, nothing is claimed.
        let (wire, seen) = fake_agent(gemini_like(|id, _p, say| say(end_turn(id))));
        let wanted = Directives { mode: Some(Mode::Plan), effort: Some(Effort::Low), model: Some("x".into()) };
        let c = connect(wire, None, None, &opening_with("", "oi", &wanted)).expect("connects");
        let _ = until_result(&c.events);
        assert!(!methods(&seen).iter().any(|m| m.starts_with("session/set_")), "{:?}", methods(&seen));
        assert_eq!(
            c.session.apply_directives(&wanted).expect("no error, just nothing applied"),
            LiveDirectives::Applied(DirectivesApplied::default())
        );
    }

    #[test]
    fn a_model_nobody_offers_is_refused_by_name() {
        let (wire, _seen) = fake_agent(offering(claude_offer(), |id, _p, say| say(end_turn(id))));
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        let _ = until_result(&c.events);
        let err = c
            .session
            .apply_directives(&Directives { mode: None, effort: None, model: Some("gpt-5".into()) })
            .expect_err("gpt-5 is not on the list");
        let err = err.to_string();
        assert!(err.contains("gpt-5") && err.contains("claude-opus-4-6"), "{err}");
    }

    /// Events up to and including the first `Result`.
    fn until_result(rx: &EventRx) -> Vec<AgentEvent> {
        let mut out = Vec::new();
        loop {
            let ev = rx.recv_timeout(Duration::from_secs(3)).expect("an event before the timeout");
            let done = matches!(ev, AgentEvent::Result(_));
            out.push(ev);
            if done {
                return out;
            }
        }
    }

    fn methods(seen: &Seen) -> Vec<String> {
        seen.lock()
            .unwrap()
            .iter()
            .filter_map(|m| match m {
                Incoming::Request { method, .. } | Incoming::Notification { method, .. } => Some(method.clone()),
                Incoming::Response { .. } => None,
            })
            .collect()
    }

    fn wait_for(seen: &Seen, pred: impl Fn(&Incoming) -> bool) -> Incoming {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(m) = seen.lock().unwrap().iter().find(|m| pred(m)) {
                return m.clone();
            }
            assert!(std::time::Instant::now() < deadline, "hark never sent what the test waits for");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn turn(events: &[AgentEvent]) -> &hark_agent::TurnResult {
        match events.last() {
            Some(AgentEvent::Result(t)) => t,
            other => panic!("expected the turn's result last, got {other:?}"),
        }
    }

    #[test]
    fn a_new_session_is_negotiated_created_and_given_its_opening_prompt() {
        let (wire, seen) = fake_agent(gemini_like(|id, _params, say| {
            say(update(chunk("olá")));
            say(update(chunk(" mundo")));
            say(end_turn(id));
        }));
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        assert_eq!(c.negotiated.info.name, "gemini-cli");
        assert!(c.negotiated.images, "the sheet comes from the wire, not from a default");

        let events = until_result(&c.events);
        assert_eq!(
            events[..3],
            [
                AgentEvent::SessionStarted { session_id: "s-1".into(), slash_commands: vec![] },
                AgentEvent::Status(AgentPhase::Writing),
                // Two chunks, ONE message: the transcript is message-sized.
                AgentEvent::AssistantText("olá mundo".into()),
            ]
        );
        let t = turn(&events);
        assert!(!t.is_error);
        assert_eq!(t.raw, "olá mundo", "raw is the last segment, so the window does not paste it twice");
        assert!(t.usage.is_empty() && t.cost_usd.is_none(), "nothing reported, nothing invented");

        assert_eq!(methods(&seen), ["initialize", "session/new", "session/prompt"]);
        let sent = seen.lock().unwrap();
        let new = sent.iter().find_map(|m| match m {
            Incoming::Request { method, params, .. } if method == "session/new" => Some(params.clone()),
            _ => None,
        });
        assert_eq!(new.as_ref().unwrap()["cwd"], "/tmp/proj");
        assert_eq!(new.as_ref().unwrap()["mcpServers"], json!([]));
        let prompt = sent.iter().find_map(|m| match m {
            Incoming::Request { method, params, .. } if method == "session/prompt" => Some(params.clone()),
            _ => None,
        });
        let prompt = prompt.unwrap();
        assert_eq!(prompt["sessionId"], "s-1");
        assert_eq!(prompt["prompt"], json!([{ "type": "text", "text": "oi" }]));
    }

    #[test]
    fn the_handshake_declines_the_file_system_and_the_terminal() {
        // Hark does not serve fs/* or terminal/*: the agent uses its own
        // tools. Saying so at initialize is what keeps it from asking.
        let (wire, seen) = fake_agent(gemini_like(|id, _p, say| say(end_turn(id))));
        let _c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        let init = wait_for(&seen, |m| matches!(m, Incoming::Request { method, .. } if method == "initialize"));
        let Incoming::Request { params, .. } = init else { unreachable!() };
        assert_eq!(params["protocolVersion"], 1);
        assert_eq!(params["clientCapabilities"]["fs"]["readTextFile"], false);
        assert_eq!(params["clientCapabilities"]["fs"]["writeTextFile"], false);
        assert_eq!(params["clientCapabilities"]["terminal"], false);
    }

    #[test]
    fn an_unauthenticated_agent_fails_the_connect_with_the_auth_code() {
        // OBSERVED: gemini 0.46.0 answers session/new with -32000 when no
        // one is logged in (spikes/acp/FINDINGS.md). The windows already
        // render "agent_auth: …" as a login card.
        let (wire, _seen) = fake_agent(|msg, say| {
            if let Incoming::Request { id, method, .. } = msg {
                match method.as_str() {
                    "initialize" => say(rpc::response(id, serde_json::from_str(GEMINI_INIT).unwrap())),
                    "session/new" => say(rpc::error_response(
                        id,
                        -32000,
                        "Gemini API key is missing or not configured.",
                    )),
                    _ => {}
                }
            }
        });
        let Err(err) = connect(wire, None, None, &opening("", "oi")) else { panic!("must fail") };
        let msg = err.to_string();
        assert!(msg.starts_with("agent_auth: "), "{msg}");
        assert!(msg.contains("API key is missing"), "the agent's own words survive: {msg}");
    }

    #[test]
    fn any_other_refusal_is_a_plain_failure_with_the_agents_words() {
        let (wire, _seen) = fake_agent(|msg, say| {
            if let Incoming::Request { id, method, .. } = msg {
                match method.as_str() {
                    "initialize" => say(rpc::response(id, serde_json::from_str(GEMINI_INIT).unwrap())),
                    "session/new" => say(rpc::error_response(id, -32603, "disk on fire")),
                    _ => {}
                }
            }
        });
        let Err(err) = connect(wire, None, None, &opening("", "oi")) else { panic!("must fail") };
        let msg = err.to_string();
        assert!(msg.starts_with("agent_failed: "), "{msg}");
        assert!(msg.contains("disk on fire"), "{msg}");
    }

    #[test]
    fn a_resume_loads_the_session_instead_of_creating_one() {
        let (wire, seen) = fake_agent(gemini_like(|id, _p, say| say(end_turn(id))));
        let c = connect(wire, None, None, &opening("old-7", "continua")).expect("connects");
        let events = until_result(&c.events);
        assert_eq!(
            events[0],
            AgentEvent::SessionStarted { session_id: "old-7".into(), slash_commands: vec![] }
        );
        let m = methods(&seen);
        assert!(m.contains(&"session/load".to_string()) && !m.contains(&"session/new".to_string()), "{m:?}");
        let load = wait_for(&seen, |m| matches!(m, Incoming::Request { method, .. } if method == "session/load"));
        let Incoming::Request { params, .. } = load else { unreachable!() };
        assert_eq!(params["sessionId"], "old-7");
        assert_eq!(params["cwd"], "/tmp/proj");
    }

    #[test]
    fn text_around_a_tool_call_arrives_as_two_messages_with_the_tool_between() {
        let (wire, _seen) = fake_agent(gemini_like(|id, _p, say| {
            say(update(chunk("vou ler")));
            say(update(json!({ "sessionUpdate": "tool_call", "toolCallId": "c1", "title": "Read",
                "kind": "read", "status": "pending", "rawInput": { "path": "/tmp/x" } })));
            say(update(json!({ "sessionUpdate": "tool_call_update", "toolCallId": "c1",
                "status": "completed",
                "content": [{ "type": "content", "content": { "type": "text", "text": "12 linhas" } }] })));
            say(update(chunk("pronto")));
            say(end_turn(id));
        }));
        let c = connect(wire, None, None, &opening("", "lê o arquivo")).expect("connects");
        let events = until_result(&c.events);
        assert_eq!(events[1], AgentEvent::Status(AgentPhase::Writing));
        assert_eq!(events[2], AgentEvent::AssistantText("vou ler".into()));
        match &events[3] {
            AgentEvent::ToolUse { name, input } => {
                assert_eq!(name, "Read");
                assert!(input.contains("/tmp/x"));
            }
            other => panic!("expected the tool call, got {other:?}"),
        }
        assert_eq!(events[4], AgentEvent::ToolResult { content: "12 linhas".into(), is_error: false });
        assert_eq!(events[5], AgentEvent::Status(AgentPhase::Writing));
        assert_eq!(events[6], AgentEvent::AssistantText("pronto".into()));
        assert_eq!(turn(&events).raw, "pronto");
    }

    #[test]
    fn a_thought_ends_the_segment_and_is_a_phase_not_text() {
        let (wire, _seen) = fake_agent(gemini_like(|id, _p, say| {
            say(update(json!({ "sessionUpdate": "agent_thought_chunk",
                "content": { "type": "text", "text": "hmm" } })));
            say(update(chunk("resposta")));
            say(end_turn(id));
        }));
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        let events = until_result(&c.events);
        assert_eq!(events[1], AgentEvent::Status(AgentPhase::Thinking));
        assert_eq!(events[2], AgentEvent::Status(AgentPhase::Writing));
        assert_eq!(events[3], AgentEvent::AssistantText("resposta".into()));
        assert!(!events.iter().any(|e| matches!(e, AgentEvent::AssistantText(t) if t.contains("hmm"))),
            "a model's scratchpad never lands in the transcript");
    }

    /// The permission dance, scripted: the agent asks (request 7) inside
    /// the turn and only finishes the turn once hark has answered.
    fn permission_agent() -> impl FnMut(&Incoming, &mut dyn FnMut(String)) + Send + 'static {
        let prompt_id: Arc<Mutex<Option<u64>>> = Arc::default();
        move |msg, say| match msg {
            Incoming::Request { id, method, .. } => match method.as_str() {
                "initialize" => say(rpc::response(id, serde_json::from_str(GEMINI_INIT).unwrap())),
                "session/new" => say(rpc::response(id, json!({ "sessionId": "s-1" }))),
                "session/prompt" => {
                    *prompt_id.lock().unwrap() = id.as_u64();
                    say(rpc::request(7, "session/request_permission", json!({
                        "sessionId": "s-1",
                        "toolCall": { "toolCallId": "c1", "title": "Bash", "kind": "execute",
                                      "rawInput": { "command": "ls -la" } },
                        "options": [
                            { "optionId": "a", "name": "Allow always", "kind": "allow_always" },
                            { "optionId": "o", "name": "Allow once", "kind": "allow_once" },
                            { "optionId": "r", "name": "Reject", "kind": "reject_once" }
                        ]
                    })));
                }
                _ => {}
            },
            // Hark answered our permission request: the turn can end.
            Incoming::Response { id: 7, .. } => {
                if let Some(pid) = *prompt_id.lock().unwrap() {
                    say(end_turn(pid));
                }
            }
            Incoming::Notification { method, .. } if method == "session/cancel" => {
                if let Some(pid) = *prompt_id.lock().unwrap() {
                    say(rpc::response(&json!(pid), json!({ "stopReason": "cancelled" })));
                }
            }
            _ => {}
        }
    }

    fn permission_event(rx: &EventRx) -> String {
        loop {
            match rx.recv_timeout(Duration::from_secs(3)).expect("an event") {
                AgentEvent::PermissionRequest { request_id, tool_name, input } => {
                    assert_eq!(tool_name, "Bash");
                    assert!(input.contains("ls -la"), "the payload is the agent's rawInput: {input}");
                    return request_id;
                }
                AgentEvent::Result(t) => panic!("the turn ended before asking: {t:?}"),
                _ => {}
            }
        }
    }

    fn answer_to(seen: &Seen, id: u64) -> Value {
        let m = wait_for(seen, |m| matches!(m, Incoming::Response { id: got, .. } if *got == id));
        let Incoming::Response { result, .. } = m else { unreachable!() };
        result.expect("a result, not an error")
    }

    #[test]
    fn a_permission_request_becomes_an_event_and_allow_picks_allow_once_never_always() {
        let (wire, seen) = fake_agent(permission_agent());
        let c = connect(wire, None, None, &opening("", "lista")).expect("connects");
        let request_id = permission_event(&c.events);
        assert!(request_id.starts_with("acp-"), "hark's own id, not the agent's: {request_id}");

        c.session.respond_permission(&request_id, PermissionDecision::Allow).expect("answers");
        let outcome = answer_to(&seen, 7);
        assert_eq!(outcome["outcome"]["outcome"], "selected");
        assert_eq!(outcome["outcome"]["optionId"], "o", "allow_once, never the standing rule");
        assert!(!turn(&until_result(&c.events)).is_error);
    }

    #[test]
    fn deny_picks_reject_once() {
        let (wire, seen) = fake_agent(permission_agent());
        let c = connect(wire, None, None, &opening("", "lista")).expect("connects");
        let request_id = permission_event(&c.events);
        c.session.respond_permission(&request_id, PermissionDecision::Deny).expect("answers");
        assert_eq!(answer_to(&seen, 7)["outcome"]["optionId"], "r");
    }

    #[test]
    fn answering_an_ask_nobody_made_is_an_error_not_a_write() {
        let (wire, seen) = fake_agent(gemini_like(|id, _p, say| say(end_turn(id))));
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        assert!(c.session.respond_permission("acp-99", PermissionDecision::Allow).is_err());
        let _ = until_result(&c.events);
        assert!(!seen.lock().unwrap().iter().any(|m| matches!(m, Incoming::Response { .. })));
    }

    #[test]
    fn interrupt_cancels_the_turn_and_answers_open_permissions_as_cancelled() {
        let (wire, seen) = fake_agent(permission_agent());
        let c = connect(wire, None, None, &opening("", "lista")).expect("connects");
        let _request_id = permission_event(&c.events);

        c.session.interrupt().expect("cancels");
        // The spec: a client that cancels MUST answer every pending
        // permission with `cancelled`, then the agent stops with `cancelled`.
        let cancelled = answer_to(&seen, 7);
        assert_eq!(cancelled["outcome"]["outcome"], "cancelled");
        let cancel = wait_for(&seen, |m| matches!(m, Incoming::Notification { method, .. } if method == "session/cancel"));
        let Incoming::Notification { params, .. } = cancel else { unreachable!() };
        assert_eq!(params["sessionId"], "s-1");
        let t = turn(&until_result(&c.events)).clone();
        assert!(!t.is_error, "a stop the user asked for is not a failure");
        assert_eq!(t.raw, "", "no prose was written, none is reported");
    }

    #[test]
    fn available_commands_become_the_sessions_slash_list() {
        let (wire, _seen) = fake_agent(gemini_like(|id, _p, say| {
            say(update(json!({ "sessionUpdate": "available_commands_update",
                "availableCommands": [
                    { "name": "compact", "description": "Compress the context" },
                    { "name": "help", "description": "Help" }
                ] })));
            say(end_turn(id));
        }));
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        let events = until_result(&c.events);
        assert!(events.contains(&AgentEvent::SessionStarted {
            session_id: "s-1".into(),
            slash_commands: vec!["compact".into(), "help".into()],
        }), "{events:?}");
    }

    #[test]
    fn an_agent_request_hark_does_not_serve_gets_method_not_found_and_the_turn_goes_on() {
        let (wire, seen) = fake_agent(gemini_like(|id, _p, say| {
            say(rpc::request(9, "fs/read_text_file", json!({ "sessionId": "s-1", "path": "/etc/hosts" })));
            say(update(chunk("segui")));
            say(end_turn(id));
        }));
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        let events = until_result(&c.events);
        assert_eq!(turn(&events).raw, "segui");
        let refusal = wait_for(&seen, |m| matches!(m, Incoming::Response { id: 9, .. }));
        let Incoming::Response { error, .. } = refusal else { unreachable!() };
        assert_eq!(error.expect("an error")["code"], -32601);
    }

    #[test]
    fn images_travel_as_image_blocks_after_the_text() {
        let (wire, seen) = fake_agent(gemini_like(|id, _p, say| say(end_turn(id))));
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        let _ = until_result(&c.events);
        c.session
            .send_text("veja", &[("image/png".into(), "AAAA".into())])
            .expect("sends");
        let _ = until_result(&c.events);
        let prompts: Vec<Value> = seen
            .lock()
            .unwrap()
            .iter()
            .filter_map(|m| match m {
                Incoming::Request { method, params, .. } if method == "session/prompt" => Some(params["prompt"].clone()),
                _ => None,
            })
            .collect();
        assert_eq!(prompts.len(), 2);
        assert_eq!(
            prompts[1],
            json!([{ "type": "text", "text": "veja" },
                   { "type": "image", "data": "AAAA", "mimeType": "image/png" }])
        );
    }

    #[test]
    fn a_second_prompt_while_one_is_in_flight_is_refused() {
        // ACP: one prompt per session at a time. The outbox queues the
        // rest; the session refuses rather than corrupting the turn.
        let (wire, _seen) = fake_agent(gemini_like(|_id, _p, _say| { /* never answers */ }));
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        let err = c.session.send_text("mais", &[]).expect_err("refused");
        assert!(err.to_string().contains("in flight"), "{err}");
    }

    #[test]
    fn junk_on_stdout_is_ignored() {
        let (wire, _seen) = fake_agent(|msg, say| {
            if let Incoming::Request { id, method, .. } = msg {
                match method.as_str() {
                    "initialize" => {
                        say("Loading extensions...".into());
                        say(rpc::response(id, serde_json::from_str(GEMINI_INIT).unwrap()));
                    }
                    "session/new" => say(rpc::response(id, json!({ "sessionId": "s-1" }))),
                    "session/prompt" => {
                        say("[debug] prompt received".into());
                        say(rpc::response(id, json!({ "stopReason": "end_turn" })));
                    }
                    _ => {}
                }
            }
        });
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects despite the noise");
        assert!(!turn(&until_result(&c.events)).is_error);
    }

    #[test]
    fn context_usage_rides_on_the_turn_and_cost_is_the_delta_of_a_running_total() {
        let calls = Arc::new(Mutex::new(0u32));
        let (wire, _seen) = fake_agent(gemini_like(move |id, _p, say| {
            let n = { let mut c = calls.lock().unwrap(); *c += 1; *c };
            let amount = if n == 1 { 0.01 } else { 0.03 };
            say(update(json!({ "sessionUpdate": "usage_update", "used": 1200 * n as u64, "size": 32000,
                "cost": { "amount": amount, "currency": "USD" } })));
            say(end_turn(id));
        }));
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        let first = turn(&until_result(&c.events)).clone();
        assert_eq!(
            first.usage,
            vec![ModelUsage {
                model: "gemini".into(),
                usage: TokenUsage { input: 1200, ..Default::default() },
                cost_usd: Some(0.01),
                context_window: Some(32000),
            }]
        );
        assert_eq!(first.cost_usd, Some(0.01));

        c.session.send_text("de novo", &[]).expect("sends");
        let second = turn(&until_result(&c.events)).clone();
        assert_eq!(second.usage[0].usage.input, 2400, "used is this turn's prompt size");
        assert!((second.cost_usd.unwrap() - 0.02).abs() < 1e-9, "0.03 total − 0.01 before = this turn");
    }

    #[test]
    fn a_later_usage_update_without_cost_does_not_erase_the_turns_cost() {
        // claude-agent-acp prices the turn on the SDK result, then keeps
        // sending usage_update for context (and on rate-limit events) with
        // no cost at all. The last reading wins for used/size; the cost is
        // the last one anyone stated.
        let (wire, _seen) = fake_agent(gemini_like(|id, _p, say| {
            say(update(json!({ "sessionUpdate": "usage_update", "used": 27_000, "size": 200_000,
                "cost": { "amount": 0.28, "currency": "USD" } })));
            say(update(json!({ "sessionUpdate": "usage_update", "used": 27_400, "size": 200_000 })));
            say(end_turn(id));
        }));
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        let t = turn(&until_result(&c.events)).clone();
        assert_eq!(t.cost_usd, Some(0.28), "the priced reading survives the unpriced one");
        assert_eq!(t.usage[0].usage.input, 27_400, "context still follows the latest reading");
        assert_eq!(t.usage[0].cost_usd, Some(0.28));
    }

    static NO_LIMITS: hark_agent::SpawnLimits = hark_agent::SpawnLimits { max_budget_usd: None, max_turns: None };

    /// A lean one-shot question, the way the ask lane opens one.
    fn lean_opening<'a>(instruction: &'a str, system: &'a str) -> Opening<'a> {
        Opening { lean: Some(LeanAsk { system_prompt: system }), ..opening("", instruction) }
    }

    fn session_new_params(seen: &Seen) -> Value {
        requests(seen, "session/new").into_iter().next().expect("session/new was sent")
    }

    fn first_prompt_text(seen: &Seen) -> String {
        let p = requests(seen, "session/prompt").into_iter().next().expect("prompt was sent");
        p["prompt"][0]["text"].as_str().unwrap_or_default().to_string()
    }

    #[test]
    fn a_lean_ask_on_the_claude_adapter_replaces_the_system_prompt_and_strips_settings() {
        // Recorded: one word cost 28.5k tokens of Claude Code system prompt
        // written to cache. The adapter takes a replacement system prompt
        // and SDK options in session/new _meta; with them a one-shot ask
        // carries our persona and nothing else.
        let (wire, seen) = fake_agent(claude_like(|id, _p, say| say(end_turn(id))));
        let c = connect(wire, None, None, &lean_opening("qual é a pendência?", "Você é o Hark."))
            .expect("connects");
        let _ = until_result(&c.events);

        let meta = &session_new_params(&seen)["_meta"];
        assert_eq!(meta["systemPrompt"], "Você é o Hark.");
        assert_eq!(meta["disableBuiltInTools"], true);
        let options = &meta["claudeCode"]["options"];
        assert_eq!(options["settingSources"], json!([]));
        assert_eq!(options["tools"], json!([]));
        assert_eq!(options["maxTurns"], 1);
        // The persona went in _meta, so it is not repeated in the prompt.
        assert_eq!(first_prompt_text(&seen), "qual é a pendência?");
    }

    #[test]
    fn a_lean_ask_on_another_agent_keeps_the_system_prompt_in_the_prompt() {
        // gemini knows no such _meta: nothing it would not understand goes
        // on the wire, and the persona rides the prompt as before.
        let (wire, seen) = fake_agent(gemini_like(|id, _p, say| say(end_turn(id))));
        let c = connect(wire, None, None, &lean_opening("qual é a pendência?", "Você é o Hark."))
            .expect("connects");
        let _ = until_result(&c.events);
        assert!(session_new_params(&seen).get("_meta").is_none());
        let text = first_prompt_text(&seen);
        assert!(text.starts_with("Você é o Hark."), "{text}");
        assert!(text.ends_with("qual é a pendência?"), "{text}");
    }

    #[test]
    fn a_chat_on_the_claude_adapter_hands_it_the_ceiling_too() {
        // Not lean: a real chat keeps Claude Code's own prompt and tools,
        // and only the limits ride _meta — the SDK enforces them as the
        // CLI's --max-budget-usd / --max-turns would.
        let (wire, seen) = fake_agent(claude_like(|id, _p, say| say(end_turn(id))));
        let limits = hark_agent::SpawnLimits { max_budget_usd: Some(2.0), max_turns: Some(30) };
        let c = connect(wire, None, None, &Opening { limits: &limits, ..opening("", "oi") }).expect("connects");
        let _ = until_result(&c.events);
        let meta = &session_new_params(&seen)["_meta"];
        assert_eq!(meta["claudeCode"]["options"]["maxBudgetUsd"], 2.0);
        assert_eq!(meta["claudeCode"]["options"]["maxTurns"], 30);
        assert!(meta.get("systemPrompt").is_none(), "a chat keeps the agent's own prompt");
        assert!(meta["claudeCode"]["options"].get("tools").is_none(), "and its tools");
    }

    /// An agent that reports a running cost of `total` on every prompt and
    /// only ends the prompt once hark cancels it — the shape of a turn that
    /// would keep spending if nobody stopped it.
    fn spender(total: f64, tool_calls: usize) -> impl FnMut(&Incoming, &mut dyn FnMut(String)) + Send + 'static {
        let mut in_flight: Option<u64> = None;
        move |msg, say| match msg {
            Incoming::Request { id, method, .. } => match method.as_str() {
                "initialize" => say(rpc::response(id, serde_json::from_str(GEMINI_INIT).unwrap())),
                "session/new" => say(rpc::response(id, json!({ "sessionId": "s-1" }))),
                "session/prompt" => {
                    in_flight = id.as_u64();
                    for n in 0..tool_calls {
                        say(update(json!({ "sessionUpdate": "tool_call", "toolCallId": format!("c{n}"),
                            "title": "Bash", "kind": "execute", "status": "pending" })));
                    }
                    say(update(json!({ "sessionUpdate": "usage_update", "used": 1000, "size": 200000,
                        "cost": { "amount": total, "currency": "USD" } })));
                }
                _ => say(rpc::error_response(id, -32601, "not in this script")),
            },
            Incoming::Notification { method, .. } if method == "session/cancel" => {
                if let Some(id) = in_flight.take() {
                    say(rpc::response(&json!(id), json!({ "stopReason": "cancelled" })));
                }
            }
            _ => {}
        }
    }

    #[test]
    fn the_budget_ceiling_cancels_the_turn_and_says_so() {
        // The CLI's --max-budget-usd has no ACP equivalent: the client is
        // the ceiling. Past the budget, hark cancels and the turn ends as
        // an error that names both numbers — never a silent spend.
        let (wire, seen) = fake_agent(spender(2.5, 0));
        let limits = hark_agent::SpawnLimits { max_budget_usd: Some(2.0), max_turns: None };
        let c = connect(wire, None, None, &Opening { limits: &limits, ..opening("", "oi") }).expect("connects");
        let t = turn(&until_result(&c.events)).clone();
        assert!(methods(&seen).iter().any(|m| m == "session/cancel"), "{:?}", methods(&seen));
        assert!(t.is_error);
        assert!(t.raw.contains("teto") && t.raw.contains("2.00") && t.raw.contains("2.50"), "{}", t.raw);
        assert_eq!(t.cost_usd, Some(2.5), "the money spent is still on the record");
    }

    #[test]
    fn the_tool_call_ceiling_cancels_after_the_limit() {
        let (wire, seen) = fake_agent(spender(0.01, 3));
        let limits = hark_agent::SpawnLimits { max_budget_usd: None, max_turns: Some(2) };
        let c = connect(wire, None, None, &Opening { limits: &limits, ..opening("", "oi") }).expect("connects");
        let t = turn(&until_result(&c.events)).clone();
        assert!(methods(&seen).iter().any(|m| m == "session/cancel"));
        assert!(t.is_error && t.raw.contains("teto"), "{}", t.raw);
    }

    #[test]
    fn under_the_ceiling_nothing_is_cancelled() {
        let (wire, seen) = fake_agent(gemini_like(|id, _p, say| {
            say(update(json!({ "sessionUpdate": "usage_update", "used": 100, "size": 200000,
                "cost": { "amount": 0.5, "currency": "USD" } })));
            say(end_turn(id));
        }));
        let limits = hark_agent::SpawnLimits { max_budget_usd: Some(2.0), max_turns: Some(30) };
        let c = connect(wire, None, None, &Opening { limits: &limits, ..opening("", "oi") }).expect("connects");
        let t = turn(&until_result(&c.events)).clone();
        assert!(!t.is_error);
        assert!(!methods(&seen).iter().any(|m| m == "session/cancel"));
    }

    #[test]
    fn the_prompt_answers_token_breakdown_beats_the_context_reading() {
        // Recorded: usage_update says used=28586 (the context), and the
        // prompt response says what the turn itself cost in tokens —
        // 10 in, 72 out, 28504 written to cache. The ledger wants the
        // latter; the ring wants the former.
        let (wire, _seen) = fake_agent(gemini_like(|id, _p, say| {
            say(update(json!({ "sessionUpdate": "usage_update", "used": 28586, "size": 200000,
                "cost": { "amount": 0.057378, "currency": "USD" } })));
            say(end_turn_with_usage(id));
        }));
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        let t = turn(&until_result(&c.events)).clone();
        assert_eq!(
            t.usage[0].usage,
            TokenUsage { input: 10, output: 72, cache_read: 0, cache_created: 28504 }
        );
        assert_eq!(t.usage[0].context_window, Some(200000));
        assert!((t.cost_usd.unwrap() - 0.057378).abs() < 1e-9);
        // The response's _meta.quota names the model that actually ran:
        // the footer signs "haiku-4-5-20251001", not the agent id.
        assert_eq!(t.model.as_deref(), Some("claude-haiku-4-5-20251001"));
        assert_eq!(t.usage[0].model, "claude-haiku-4-5-20251001");
    }

    #[test]
    fn a_breakdown_with_no_usage_update_still_makes_a_row() {
        let (wire, _seen) = fake_agent(gemini_like(|id, _p, say| say(end_turn_with_usage(id))));
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        let t = turn(&until_result(&c.events)).clone();
        assert_eq!(t.usage.len(), 1);
        assert_eq!(t.usage[0].usage.output, 72);
        assert_eq!(t.usage[0].context_window, None, "nobody said how big the window is");
        assert_eq!(t.cost_usd, None, "nobody priced it");
    }

    #[test]
    fn a_turn_that_ends_in_error_says_so() {
        let (wire, _seen) = fake_agent(gemini_like(|id, _p, say| {
            say(rpc::error_response(&json!(id), -32603, "model exploded"));
        }));
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        let t = turn(&until_result(&c.events)).clone();
        assert!(t.is_error);
        assert!(t.raw.contains("model exploded"), "{}", t.raw);
    }

    #[test]
    fn the_event_stream_closes_when_hark_hangs_up() {
        let (wire, _seen) = fake_agent(gemini_like(|id, _p, say| say(end_turn(id))));
        let c = connect(wire, None, None, &opening("", "oi")).expect("connects");
        let _ = until_result(&c.events);
        c.session.shutdown();
        // The fake sees EOF, exits, and its side of the pipe closes ours.
        assert!(
            matches!(c.events.recv_timeout(Duration::from_secs(3)), Err(std::sync::mpsc::RecvTimeoutError::Disconnected)),
            "the driver learns the session ended from the stream closing"
        );
        assert_eq!(c.session.exit_report(), (None, String::new()));
        assert_eq!(c.session.pid(), None);
    }
}

#[cfg(test)]
mod forking {
    //! "Open in parallel" over ACP: `session/fork` (announced in
    //! `sessionCapabilities.fork` by claude-agent-acp 0.76 and codex-acp
    //! 1.11) opens a NEW session on the old one's history.
    use super::fake::{end_turn, fake_agent, gemini_like, CLAUDE_INIT, CLAUDE_NEW};
    use super::{connect, Opening};
    use crate::rpc::{self, Incoming};
    use hark_agent::AgentEvent;
    use hark_core::domain::directives::Directives;
    use serde_json::{json, Value};

    static NONE: Directives = Directives { mode: None, effort: None, model: None };
    static NO_LIMITS: hark_agent::SpawnLimits = hark_agent::SpawnLimits { max_budget_usd: None, max_turns: None };

    fn fork_of<'a>(session_id: &'a str) -> Opening<'a> {
        Opening {
            agent: "claude-acp",
            cwd: std::path::Path::new("/tmp/proj"),
            session_id,
            instruction: "continua daqui, em paralelo",
            images: &[],
            memory_file: None,
            directives: &NONE,
            limits: &NO_LIMITS,
            lean: None,
            fork: true,
        }
    }

    /// An agent that handshakes like claude-agent-acp and answers
    /// session/fork with a fresh id and the same offer as session/new.
    fn forking(
        mut on_prompt: impl FnMut(u64, &Value, &mut dyn FnMut(String)) + Send + 'static,
    ) -> impl FnMut(&Incoming, &mut dyn FnMut(String)) + Send + 'static {
        move |msg, say| {
            if let Incoming::Request { id, method, params } = msg {
                match method.as_str() {
                    "initialize" => say(rpc::response(id, serde_json::from_str(CLAUDE_INIT).unwrap())),
                    "session/fork" => {
                        let mut answer: Value = serde_json::from_str(CLAUDE_NEW).unwrap();
                        answer["sessionId"] = json!("s-new");
                        say(rpc::response(id, answer))
                    }
                    "session/set_mode" | "session/set_config_option" => say(rpc::response(id, json!({}))),
                    "session/prompt" => on_prompt(id.as_u64().unwrap(), params, say),
                    _ => say(rpc::error_response(id, -32601, "not in this script")),
                }
            }
        }
    }

    fn sent(seen: &super::fake::Seen, method: &str) -> Vec<Value> {
        seen.lock()
            .unwrap()
            .iter()
            .filter_map(|m| match m {
                Incoming::Request { method: got, params, .. } if got == method => Some(params.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_fork_works_on_a_new_session_grown_from_the_old_ones_history() {
        let (wire, seen) = fake_agent(forking(|id, _p, say| say(end_turn(id))));
        let connected = connect(wire, None, None, &fork_of("s-old")).expect("connects");
        let started = connected
            .events
            .iter()
            .find_map(|e| match e {
                AgentEvent::SessionStarted { session_id, .. } => Some(session_id),
                _ => None,
            })
            .expect("a session started");
        // The window follows the NEW id: the mirror, the ledger, the card.
        assert_eq!(started, "s-new");
        let forks = sent(&seen, "session/fork");
        assert_eq!(forks.len(), 1, "one fork request: {forks:?}");
        assert_eq!(forks[0]["sessionId"], "s-old");
        assert_eq!(forks[0]["cwd"], "/tmp/proj");
        assert!(sent(&seen, "session/load").is_empty(), "a fork is not a resume");
        assert!(sent(&seen, "session/new").is_empty(), "a fork is not a blank session");
        assert!(connected.negotiated.caps.fork);
        connected.session.shutdown();
    }

    #[test]
    fn a_fork_on_an_agent_that_does_not_announce_it_is_refused_by_name() {
        // gemini 0.46 announces no sessionCapabilities.fork. Loading the old
        // session instead would not be a fork — the driver gates the banner
        // on caps.fork, and the plugin refuses rather than pretend.
        let (wire, seen) = fake_agent(gemini_like(|id, _p, say| say(end_turn(id))));
        let err = match connect(wire, None, None, &fork_of("s-old")) {
            Err(err) => err.to_string(),
            Ok(_) => panic!("a fork on gemini 0.46 must be refused"),
        };
        assert!(err.contains("fork"), "{err}");
        assert!(sent(&seen, "session/load").is_empty() && sent(&seen, "session/fork").is_empty(), "{err}");
    }
}

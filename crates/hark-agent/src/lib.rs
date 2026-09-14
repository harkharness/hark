//! The Hark agent-plugin contract.
//!
//! Hark's core never talks to a specific agent CLI: a *plugin* does, and it
//! translates whatever that CLI emits into this vocabulary. The contract is
//! deliberately an event language plus a capability sheet — the lesson from
//! other harnesses is that the seam that matters is the event stream, not a
//! config format. The first plugin wraps the Claude Code CLI; a Gemini (or
//! any other) plugin implements the same types and the whole product —
//! transcript, permissions, board, ledger, voice — keeps working.
//!
//! v1 is in-process (plugins are Rust crates composed at build time). The
//! out-of-process protocol (ndjson over stdio, "HAP") serializes exactly
//! these types; see docs/PLUGINS.md.

use serde::{Deserialize, Serialize};

/// Token counters of one model in one turn. This is the raw material of
/// the spend ledger: without measuring, nothing can be saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_created: u64,
}

/// Per-model usage of a turn. `cost_usd` is optional by design: backends
/// that don't price their turns still report tokens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelUsage {
    pub model: String,
    pub usage: TokenUsage,
    pub cost_usd: Option<f64>,
    /// The model's context window, when the backend reports it.
    pub context_window: Option<u64>,
}

/// Final outcome of one turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurnResult {
    pub is_error: bool,
    /// Structured reply as raw JSON, when the run used a response schema.
    /// The product layer decides the shape (Hark parses it as a VoiceReply).
    pub reply: Option<serde_json::Value>,
    /// Raw result string as emitted by the backend (error text or raw JSON).
    pub raw: String,
    pub cost_usd: Option<f64>,
    pub duration_ms: Option<u64>,
    /// Main model that produced the turn.
    pub model: Option<String>,
    /// Token usage per model (empty only when the backend reported nothing).
    pub usage: Vec<ModelUsage>,
}

/// Subscription/quota window signal, when the backend has one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitInfo {
    /// allowed | allowed_warning | rejected
    pub status: String,
    /// Epoch seconds when the current window resets.
    pub resets_at: Option<u64>,
    /// Window kind (e.g. five_hour, seven_day).
    pub kind: Option<String>,
    pub overage: bool,
}

/// One event from a running agent session, reduced to what Hark reacts to.
/// This is the heart of the contract: a plugin's only real job is turning
/// its CLI's output into this stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentEvent {
    /// The agent called a tool; narrated in the UI/TTS while waiting.
    ToolUse { name: String, input: String },
    /// Agent prose (live transcript between tool calls).
    AssistantText(String),
    /// A tool finished; shown in the live transcript.
    ToolResult { content: String, is_error: bool },
    /// Turn finished.
    Result(TurnResult),
    /// The backend asks whether a tool may run. Requires the
    /// `permissions` capability.
    PermissionRequest {
        request_id: String,
        tool_name: String,
        input: String,
    },
    /// The backend announced which session this process writes to, and
    /// (optionally) the slash commands that session accepts.
    SessionStarted {
        session_id: String,
        slash_commands: Vec<String>,
    },
    /// Quota window status, when the backend emits one.
    RateLimit(RateLimitInfo),
    /// What the agent is doing RIGHT NOW — no content, just the phase.
    /// A turn can spend a minute reasoning before it emits a single word,
    /// and a window with nothing on screen reads as a window that broke.
    /// A backend that cannot report this simply never sends it.
    Status(AgentPhase),
    /// Anything Hark doesn't react to (kept so streams stay auditable).
    Ignored,
}

/// Coarse phases of a running turn. Deliberately few: each one must be
/// something a backend can state as fact, never guessed from a timer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPhase {
    /// Request in flight, nothing back yet.
    Requesting,
    /// Reasoning tokens arriving.
    Thinking,
    /// Prose arriving.
    Writing,
}

/// User's verdict on a permission request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionDecision {
    Allow,
    Deny,
}

/// Hard caps applied to a spawned agent session (part of the spawn spec;
/// a backend that can't enforce one simply ignores it and says so in docs).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct SpawnLimits {
    pub max_budget_usd: Option<f64>,
    pub max_turns: Option<u32>,
}

/// One fact extracted from a backend's on-disk session history (the
/// `history` capability). Parsing the backend's format is the plugin's
/// job; this is the neutral shape the indexer consumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionEvent {
    UserPrompt {
        ts: String,
        text: String,
        cwd: Option<String>,
        git_branch: Option<String>,
    },
    /// Any agent output; only the timestamp matters (session freshness).
    Activity { ts: String },
    /// Agent output carrying token usage: the retroactive spend trail
    /// (history files rarely carry USD, only tokens).
    AssistantUsage {
        ts: String,
        /// Dedup key: the same message can repeat across lines.
        request_id: Option<String>,
        model: Option<String>,
        usage: TokenUsage,
        is_sidechain: bool,
    },
    /// Context compaction record: how many tokens the window dropped.
    CompactBoundary { ts: String, pre_tokens: u64, post_tokens: u64 },
    /// User-assigned session title.
    Title(String),
}

/// What a backend can actually do. The UI degrades feature by feature:
/// no `cost_reporting` hides USD, no `permissions` hides approval cards,
/// no `history` blanks the session browser — nothing else breaks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// Resuming an existing session by id.
    pub resume: bool,
    /// Interactive tool-permission requests (PermissionRequest events).
    pub permissions: bool,
    /// Structured one-shot answers under a JSON schema (quick_ask).
    pub structured_output: bool,
    /// Turns carry USD cost.
    pub cost_reporting: bool,
    /// Session history on disk that Hark can index.
    pub history: bool,
    /// Listing live sessions started outside Hark.
    pub live_list: bool,
    /// Slash commands (e.g. /compact) sent as plain turn text.
    pub slash_commands: bool,
    /// Memory file the backend auto-loads from a working dir, if any
    /// (the Claude CLI loads "CLAUDE.md"); Hark writes the persona there.
    pub memory_file: Option<String>,
    /// Tool names whose input is a shell command (feeds the production
    /// gate — kubectl/terraform/etc. never auto-approve).
    pub shell_tools: Vec<String>,
    /// Forking a session: resume the history into a NEW session id,
    /// leaving the original untouched (claude --fork-session).
    #[serde(default)]
    pub fork: bool,
    /// The permission-mode directive reaches the agent (a CLI flag, or ACP
    /// `session/set_mode` when the agent offers modes). Without it a pill
    /// takes the click and changes nothing, which is worse than a pill
    /// that says it cannot.
    #[serde(default)]
    pub directive_mode: bool,
    /// The model directive reaches the agent (a flag, or an ACP config
    /// option categorised `model`).
    #[serde(default)]
    pub directive_model: bool,
    /// The effort directive reaches the agent (a flag, or an ACP config
    /// option categorised `thought_level`).
    #[serde(default)]
    pub directive_effort: bool,
}

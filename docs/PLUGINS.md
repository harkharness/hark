# Hark agent plugins

Hark's core never talks to a specific agent CLI. A **plugin** does, and it
translates that CLI into the contract defined by the `hark-agent` crate.
The first plugin wraps the Claude Code CLI (`hark-plugin-claude`); a Gemini
(or any other) plugin implements the same contract and the whole product —
transcript, permissions, board, ledger, voice — keeps working.

The design lesson borrowed from other harnesses (notably deepseek-harness's
`ctx.llm` seam): **the contract is an event vocabulary plus a capability
sheet**, not a config format. If you can turn your CLI's output into
`AgentEvent`s, you have a plugin.

## The contract (`crates/hark-agent`)

Types every plugin speaks:

| Type | Role |
|---|---|
| `AgentEvent` | The event stream: `ToolUse`, `AssistantText`, `ToolResult`, `Result(TurnResult)`, `PermissionRequest`, `SessionStarted`, `RateLimit`, `Ignored` |
| `TurnResult` | End of turn: `is_error`, `raw`, `reply` (raw JSON when a schema constrained the answer), `cost_usd: Option`, per-model `usage` |
| `ModelUsage` / `TokenUsage` | The ledger's raw material; USD optional, tokens not |
| `SessionEvent` | Neutral history facts (`UserPrompt`, `AssistantUsage`, `CompactBoundary`, `Title`) the indexer consumes |
| `SpawnLimits` | Hard caps (`max_budget_usd`, `max_turns`) |
| `Capabilities` | What the backend can do; the UI degrades feature by feature |

`Capabilities` fields and their UI meaning:

- `permissions` — approval cards + voice "pode/nega" flow
- `cost_reporting` — USD anywhere (no → tokens only)
- `structured_output` — the voice ask + dispatch gate (schema-constrained
  one-shot)
- `history` — session browser, resume, retroactive token ledger
- `live_list` — "what's running" from sessions started outside Hark
- `slash_commands` — /compact & friends sent as plain turn text
- `memory_file` — file name the backend auto-loads from its cwd (Claude:
  `CLAUDE.md`); Hark writes the assistant's persona there
- `shell_tools` — tool names whose input is a shell command; feeds the
  production gate (kubectl/terraform/etc. never auto-approve)

Seams the core exposes (in `hark-core::ports` today):

- `AgentRunner::ask(TurnRequest)` — schema-constrained one-shot. The voice
  ask AND the dispatch gate both ride it; `TurnRequest` carries prompt,
  images, model, system_prompt, schema, effort.
- `HistoryIndexer::refresh(projects_dir, store)` — parse your history into
  `SessionEvent`s; the core owns the SQLite index and the spend ledger.

The persistent worker (spawn/resume/send/permissions over a long-lived
process) still lives as concrete types in `hark-plugin-claude::worker`;
lifting it into a `AgentSession` trait is the next step of this refactor.

## What lives in a plugin

Everything that knows the backend. For Claude Code that is:

- `stream.rs` — stream-json → `AgentEvent`, plus the writers
  (`user_message` with image blocks, `permission_response`)
- `worker.rs` — persistent + one-shot processes, CLI flags, `--resume`
- `history.rs` + `log.rs` — `~/.claude/projects/**.jsonl` → `SessionEvent`
- `live.rs` — `claude agents --json`
- `bridge.rs` + `statusline.rs` — the statusline bridge (subscription %)
- `eco.rs` — community eco-tools detection and per-worker env injection

## Config

Plugin-specific keys stay in the user config under their historical names
for now (`claude_bin`, `projects_dir`). When a second plugin lands they move
under `[agent.<plugin>]` tables with `[agent] plugin = "claude"` selecting
the active one.

## Out-of-process plugins (HAP — future)

For non-Rust/community plugins the same contract crosses a process
boundary as **HAP**: newline-delimited JSON over stdio.

- Handshake: plugin prints `{"hello": {"name", "version", "capabilities"}}`.
- Events out: each line is one serialized `AgentEvent` (serde_json).
- Commands in: `{"send": {...}} | {"approve": {"request_id", "allow"}} |
  {"interrupt"} | {"shutdown"} | {"quick_ask": {...}}`.
- Discovery: `~/.config/hark/plugins/<name>/plugin.toml` declaring the
  binary and args; a `plugin_host` adapter implements the same core seams
  by speaking HAP to the child process.

## TODO: extract the plugins into their own public repos

Decided 23/08/2026: plugins eventually live in a dedicated **Hark org** on
GitHub, public, one repo per plugin (plus the contract). The core app stays
private. Not now — this note is the map for when we do it.

Today `hark-plugin-claude` compiles against `hark-core`, which is exactly
what a public repo cannot depend on. The good news is how little is left:
five of its eight modules (`stream`, `log`, `bridge`, `statusline`, `eco`)
already import nothing from the core. The remaining coupling is four
imports' worth:

| Module | Needs from `hark-core` | Fix |
|---|---|---|
| `cli.rs` | `ports::{AgentRunner, TurnRequest}` | move both into `hark-agent` |
| `live.rs` | `ports::LiveSessions`, `domain::prompt::LiveSession` | move both into `hark-agent` |
| `history.rs` | `ports::{SessionStore, SpendLedger, SpendQuery, SpendGroup, HistoryIndexer}`, `domain::spend::{SpendRow, SpendKind, SpendSource}`, `domain::snapshot::SessionSummary` | move the traits + row types into `hark-agent`; the SQLite impl stays in the core |
| `worker.rs` | `domain::directives::{Directives, Mode, Effort}`, `domain::prodgate::BYPASS_DENY_RULES` | `Directives` is contract material (the spawn spec already carries it); the deny rules become a `Capabilities`-adjacent constant owned by the plugin |

`SqliteStore` shows up only in `history.rs` tests — a dev-dependency, not a
real edge.

So the extraction is: **fold the ports into `hark-agent`** (they are the
contract already, they just live in the wrong crate), flip
`hark-plugin-claude`'s dependency to `hark-agent` alone, then split:

1. `hark-agent` → public repo (the contract; versioned, semver matters).
2. `hark-plugin-claude` → public repo depending on the published
   `hark-agent`.
3. The core app keeps them as git dependencies (or path deps in a local
   workspace override during development).

Until then both crates stay in this private workspace as path deps, which
is why the boundary is enforced by the compiler today: the moment a core
type sneaks back into a plugin module, the split gets harder.
